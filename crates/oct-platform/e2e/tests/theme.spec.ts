import { test, expect } from '../fixtures';

test.describe('Theme Toggle', () => {
    test('should default to dark mode and toggle to light mode', async ({ page, server }) => {
        // 1. Navigate to the projects page
        await page.goto(`${server.url}/projects`);

        // 2. Check default state (dark mode)
        const html = page.locator('html');
        await expect(html).toHaveClass(/dark/);

        // Check that the Sun icon is visible (indicating we are in dark mode, ready to switch to light)
        const sunIcon = page.locator('button[title="Toggle Theme"] .fa-sun');
        await expect(sunIcon).toBeVisible();
        const moonIcon = page.locator('button[title="Toggle Theme"] .fa-moon');
        await expect(moonIcon).toBeHidden();

        // 3. Click the toggle button
        await page.locator('button[title="Toggle Theme"]').click();

        // 4. Assert "dark" class is removed (Light Mode)
        await expect(html).not.toHaveClass(/dark/);

        // Check icons swapped
        await expect(sunIcon).toBeHidden();
        await expect(moonIcon).toBeVisible();

        // 5. Check LocalStorage persistence
        const theme = await page.evaluate(() => localStorage.getItem('theme'));
        expect(theme).toBe('light');

        // 6. Reload and verify persistence
        await page.reload();
        await expect(html).not.toHaveClass(/dark/);
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
