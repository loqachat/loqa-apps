// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2025-2026 Loqa Contributors
/**
 * WebDriverIO config for Loqa Desktop E2E tests.
 *
 * Architecture:  Test Runner  →  tauri-driver (:4444)  →  msedgedriver  →  Tauri App (WebView2)
 *
 * Usage:
 *   1. Build the app:    cd src-tauri && cargo build
 *   2. Run tests:        cd e2e && npm test
 *
 * The test runner auto-downloads msedgedriver (matching your Edge/WebView2 version)
 * and starts tauri-driver as a child process. No manual setup needed.
 *
 * For authenticated tests, set environment variables:
 *   LOQA_TEST_TOKEN=<jwt>  LOQA_TEST_USER=<user_json>  npm test
 */
import path from "path";
import { spawn, type ChildProcess } from "child_process";

// Path to the Tauri app binary. Use release build (has frontend embedded).
// Debug builds require the dev server running on :1420.
const TAURI_BINARY = process.env.TAURI_BINARY || path.resolve(
    __dirname,
    "../src-tauri/target/release/loqa-desktop.exe"
);

let tauriDriverProcess: ChildProcess | null = null;
let edgedriverBinPath: string = "";

export const config: WebdriverIO.Config = {
    // ── Runner ──────────────────────────────────────────────
    runner: "local",
    autoCompileOpts: {
        tsNodeOpts: { project: "./tsconfig.json" },
    },

    // ── Connection to tauri-driver ──────────────────────────
    hostname: "localhost",
    port: 4444,

    // ── Test specs ──────────────────────────────────────────
    specs: ["./specs/**/*.e2e.ts"],
    maxInstances: 1, // Tauri can only run one instance at a time

    // ── Capabilities ────────────────────────────────────────
    capabilities: [
        {
            // tauri-driver uses a custom vendor key to specify the app binary
            "tauri:options": {
                application: TAURI_BINARY,
            },
        } as any,
    ],

    // ── Framework ───────────────────────────────────────────
    framework: "mocha",
    mochaOpts: {
        ui: "bdd",
        timeout: 30000, // Desktop apps take time to boot
    },

    // ── Reporters ───────────────────────────────────────────
    reporters: ["spec"],

    // ── Lifecycle Hooks ─────────────────────────────────────

    /**
     * Download msedgedriver (if needed), then start tauri-driver.
     */
    onPrepare: async function () {
        // 1. Ensure msedgedriver is downloaded
        console.log("📥 Ensuring msedgedriver is available...");
        const edgedriver = require("edgedriver");
        edgedriverBinPath = await edgedriver.download();
        console.log(`   Using: ${edgedriverBinPath}`);

        // 2. Start tauri-driver, pointing at the downloaded msedgedriver
        console.log("🚀 Starting tauri-driver on :4444...");
        tauriDriverProcess = spawn("tauri-driver", [
            "--native-driver", edgedriverBinPath,
        ], {
            stdio: ["pipe", "pipe", "pipe"],
        });

        tauriDriverProcess.stdout?.on("data", (data: Buffer) => {
            const msg = data.toString().trim();
            if (msg) console.log(`  [tauri-driver] ${msg}`);
        });

        tauriDriverProcess.stderr?.on("data", (data: Buffer) => {
            const msg = data.toString().trim();
            if (msg) console.log(`  [tauri-driver:err] ${msg}`);
        });

        // Wait for tauri-driver to be ready
        await new Promise((resolve) => setTimeout(resolve, 2000));
        console.log("✅ Ready — launching tests against Loqa Desktop");
    },

    /**
     * Stop tauri-driver after all tests complete.
     */
    onComplete: async function () {
        if (tauriDriverProcess) {
            console.log("🔻 Stopping tauri-driver...");
            tauriDriverProcess.kill();
            tauriDriverProcess = null;
        }
    },

    /**
     * Take a screenshot on test failure for debugging.
     */
    afterTest: async function (test, _context, { passed }) {
        if (!passed) {
            try {
                const timestamp = Date.now();
                const name = test.title.replace(/[^a-zA-Z0-9]/g, "_");
                await browser.saveScreenshot(`./screenshots/${name}_${timestamp}.png`);
            } catch { /* screenshot may fail if app crashed */ }
        }
    },
};
