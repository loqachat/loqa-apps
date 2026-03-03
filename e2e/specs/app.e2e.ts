// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2025-2026 Loqa Contributors
/**
 * Loqa Desktop E2E — Core application tests.
 *
 * These tests run against the actual Tauri desktop app via WebDriver.
 * They verify that the app launches, renders critical UI, and key
 * user flows work end-to-end.
 *
 * NOTE: WebView2 WebDriver only supports standard CSS selectors.
 * Custom WDIO selectors like *= (text-contains) are NOT supported.
 */

describe("Loqa Desktop App", () => {
    // ── Launch & Render ──────────────────────────────────────

    it("should launch and display the app window", async () => {
        const title = await browser.getTitle();
        expect(title).toBe("Loqa");
    });

    it("should render the root app container", async () => {
        // Wait for SolidJS to mount
        await browser.waitUntil(
            async () => {
                const el = await browser.findElement("css selector", "#root");
                return !!el;
            },
            { timeout: 10000, timeoutMsg: "#root not found within 10s" }
        );
    });

    // ── Login Page ───────────────────────────────────────────

    it("should show login page when not authenticated", async () => {
        // Clear any stored tokens so we get the login screen
        await browser.execute(() => {
            localStorage.removeItem("loqa_token");
            localStorage.removeItem("loqa_user");
        });
        // Reload
        const url = await browser.getUrl();
        await browser.url(url);
        await browser.pause(2000);

        // Look for either a form or a login-related container
        const hasForm = await browser.execute(() => {
            return !!document.querySelector("form") ||
                !!document.querySelector("[class*='login']") ||
                !!document.querySelector("[class*='Login']") ||
                !!document.querySelector("input[type='password']");
        });
        expect(hasForm).toBe(true);
    });

    it("should have email/username and password input fields", async () => {
        const hasInputs = await browser.execute(() => {
            // Login.tsx uses type="text" with placeholder "you@example.com or username"
            const loginInput = document.querySelector(".auth-card input[type='text']") ||
                document.querySelector("input[placeholder*='example.com']");
            const passInput = document.querySelector(".auth-card input[type='password']");
            return { hasLogin: !!loginInput, hasPassword: !!passInput };
        });
        expect(hasInputs.hasLogin).toBe(true);
        expect(hasInputs.hasPassword).toBe(true);
    });

    it("should have a login submit button", async () => {
        const hasButton = await browser.execute(() => {
            // Login.tsx uses .btn.btn-primary type="submit"
            const btn = document.querySelector(".auth-card button[type='submit']") ||
                document.querySelector(".auth-card .btn-primary");
            return !!btn;
        });
        expect(hasButton).toBe(true);
    });

    // ── Authenticated UI ─────────────────────────────────────
    // These tests require LOQA_TEST_TOKEN and LOQA_TEST_USER env vars.

    describe("Authenticated UI", () => {
        before(async () => {
            const testToken = process.env.LOQA_TEST_TOKEN;
            const testUser = process.env.LOQA_TEST_USER;

            if (!testToken || !testUser) {
                console.log(
                    "⚠  Skipping authenticated tests — set LOQA_TEST_TOKEN and LOQA_TEST_USER"
                );
                return;
            }

            // Inject auth token to bypass login
            await browser.execute(
                (token: string, user: string) => {
                    localStorage.setItem("loqa_token", token);
                    localStorage.setItem("loqa_user", user);
                },
                testToken,
                testUser
            );
            const url = await browser.getUrl();
            await browser.url(url);
            await browser.pause(3000);
        });

        it("should render the server bar", async () => {
            if (!process.env.LOQA_TEST_TOKEN) return;

            const exists = await browser.execute(() => {
                return !!document.querySelector(".server-bar");
            });
            expect(exists).toBe(true);
        });

        it("should render the home icon in server bar", async () => {
            if (!process.env.LOQA_TEST_TOKEN) return;

            const exists = await browser.execute(() => {
                return !!document.querySelector(".server-icon.home");
            });
            expect(exists).toBe(true);
        });

        it("should render the user panel at the bottom", async () => {
            if (!process.env.LOQA_TEST_TOKEN) return;

            const exists = await browser.execute(() => {
                return !!document.querySelector(".user-panel");
            });
            expect(exists).toBe(true);
        });

        it("should have a working settings gear button", async () => {
            if (!process.env.LOQA_TEST_TOKEN) return;

            const exists = await browser.execute(() => {
                return !!document.querySelector(".user-panel-settings-btn");
            });
            expect(exists).toBe(true);
        });

        it("should show desktop settings dropdown (Tauri only)", async () => {
            if (!process.env.LOQA_TEST_TOKEN) return;

            const hasTrigger = await browser.execute(() => {
                return !!document.querySelector(".desktop-settings-trigger");
            });
            if (!hasTrigger) {
                console.log("⚠  Desktop settings trigger not found — not in Tauri env");
                return;
            }

            // Click trigger to open dropdown
            await browser.execute(() => {
                const btn = document.querySelector(".desktop-settings-trigger") as HTMLElement;
                btn?.click();
            });
            await browser.pause(300);

            const isOpen = await browser.execute(() => {
                const dd = document.querySelector(".desktop-settings-dropdown");
                return dd ? dd.textContent?.includes("Desktop Settings") : false;
            });
            expect(isOpen).toBe(true);

            // Close it
            await browser.execute(() => {
                const btn = document.querySelector(".desktop-settings-trigger") as HTMLElement;
                btn?.click();
            });
            await browser.pause(200);
        });

        it("should show DM list when on home view", async () => {
            if (!process.env.LOQA_TEST_TOKEN) return;

            // Click home icon
            await browser.execute(() => {
                const home = document.querySelector(".server-icon.home") as HTMLElement;
                home?.click();
            });
            await browser.pause(1000);

            const hasDMs = await browser.execute(() => {
                return !!document.querySelector(".dm-list") ||
                    !!document.querySelector(".dm-empty") ||
                    !!document.querySelector("[class*='dm']");
            });
            expect(hasDMs).toBe(true);
        });
    });

    // ── Window Controls ──────────────────────────────────────

    it("should be resizable", async () => {
        const size = await browser.getWindowSize();
        expect(size.width).toBeGreaterThan(0);
        expect(size.height).toBeGreaterThan(0);

        // Resize and verify
        await browser.setWindowSize(1024, 768);
        const newSize = await browser.getWindowSize();
        expect(newSize.width).toBe(1024);
        expect(newSize.height).toBe(768);

        // Restore original size
        await browser.setWindowSize(size.width, size.height);
    });
});
