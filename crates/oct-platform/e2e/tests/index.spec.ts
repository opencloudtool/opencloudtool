import { test, expect } from '../fixtures';

test('has title and main layout', async ({ page }) => {
  await expect(page).toHaveTitle(/Project Config/);
  await expect(page.getByRole('button', { name: 'Genesis' })).toBeVisible();

  await expect(page.locator('aside')).toBeVisible();
});
