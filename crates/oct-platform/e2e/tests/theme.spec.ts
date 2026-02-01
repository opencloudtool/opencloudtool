import { test, expect } from '../fixtures';

test.describe('Theme Toggle', () => {
    test('should default to dark mode when system prefers dark', async ({ page, server }) => {
        // Enforce system preference: dark
        await page.emulateMedia({ colorScheme: 'dark' });

        // 1. Navigate to the projects page
        await page.goto(`${server.url}/projects`);

        // 2. Check default state (dark mode)
        const html = page.locator('html');
        await expect(html).toHaveClass(/dark/);

        // Check that the Sun icon is visible
        const sunIcon = page.locator('button[title="Toggle Theme"] .fa-sun');
        await expect(sunIcon).toBeVisible();

        // 3. Click the toggle button to switch to light
        await page.locator('button[title="Toggle Theme"]').click();

        // 4. Assert "dark" class is removed (Light Mode)
        await expect(html).not.toHaveClass(/dark/);

        // 5. Check LocalStorage persistence (should override system)
        const theme = await page.evaluate(() => localStorage.getItem('theme'));
        expect(theme).toBe('light');
    });

    test('should default to light mode when system prefers light', async ({ page, server }) => {
        // Enforce system preference: light
        await page.emulateMedia({ colorScheme: 'light' });

        await page.goto(`${server.url}/projects`);

        const html = page.locator('html');
        // Should NOT have dark class
        await expect(html).not.toHaveClass(/dark/);

        // Moon icon should be visible
        const moonIcon = page.locator('button[title="Toggle Theme"] .fa-moon');
        await expect(moonIcon).toBeVisible();
    });

    test('should toggle back to dark mode', async ({ page, server }) => {
        await page.goto(`${server.url}/projects`);

        // Force light mode first
        await page.evaluate(() => {
            localStorage.setItem('theme', 'light');
            document.documentElement.classList.remove('dark');
        });
        await page.reload();

        // Click toggle
        await page.locator('button[title="Toggle Theme"]').click();

        // Assert dark mode
        await expect(page.locator('html')).toHaveClass(/dark/);

        const theme = await page.evaluate(() => localStorage.getItem('theme'));
        expect(theme).toBe('dark');
    });
});
