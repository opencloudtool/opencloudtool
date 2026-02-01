import { test, expect } from '../fixtures';

test('can run actions', async ({ page }) => {
  const outputEl = page.locator('#log-output');
  const closeBtn = page.locator('button').filter({ has: page.locator('.fa-xmark') });

  // Genesis
  await page.getByRole('button', { name: 'Genesis' }).click();
  await expect(outputEl).toContainText('Genesis completed successfully');
  await closeBtn.click();

  // Apply
  await page.getByRole('button', { name: 'Apply' }).click();
  await expect(outputEl).toContainText('Apply completed successfully');
  await closeBtn.click();

  // Destroy
  await page.getByRole('button', { name: 'Destroy' }).click();
  await expect(outputEl).toContainText('Destroy completed successfully');
  await closeBtn.click();
});
