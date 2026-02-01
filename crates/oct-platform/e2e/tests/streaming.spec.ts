import { test, expect } from '../fixtures';

test('action buttons open log console and stream logs', async ({ page }) => {
  await page.getByRole('button', { name: 'Genesis' }).click();

  const consoleEl = page.locator('#log-console');
  await expect(consoleEl).toBeVisible();

  const outputEl = page.locator('#log-output');
  await expect(outputEl).toContainText('Initializing stream...');

  // "Genesis completed successfully!" is logged in run_genesis
  await expect(outputEl).toContainText('Genesis completed successfully!', { timeout: 10000 });

  await page.locator('button').filter({ has: page.locator('.fa-xmark') }).click();
  await expect(consoleEl).not.toBeVisible();
});
