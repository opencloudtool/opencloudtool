import { expect, test } from "../fixtures";

test("can manage environment variables", async ({ page }) => {
    await page.getByRole("button", { name: "Edit Config" }).click();

    await page.getByRole("button", { name: "Add Service" }).click();
    await expect(page.locator('input[value="service_1"]')).toBeVisible();

    await page.getByRole("button", { name: "Add Env Var" }).click();

    await expect(page.locator('input[value="NEW_VAR"]')).toBeVisible();
    await expect(page.locator('input[value="value"]')).toBeVisible();

    const keyInput = page.locator('input[value="NEW_VAR"]');
    const valInput = page.locator('input[value="value"]');

    await keyInput.fill("DB_HOST");
    await valInput.fill("localhost");

    await page.getByRole("button", { name: "Save Changes" }).click();

    const serviceCard = page
        .locator("#services-grid .bg-surface")
        .filter({ hasText: "service_1" });
    await expect(serviceCard.getByText("DB_HOST")).toBeVisible();
    await expect(serviceCard.getByText("localhost")).toBeVisible();

    await page.getByRole("button", { name: "Edit Config" }).click();
    await expect(page.locator('input[value="DB_HOST"]')).toBeVisible();
    await expect(page.locator('input[value="localhost"]')).toBeVisible();

    // Delete Env Var
    await page.locator("#edit-config-form .fa-xmark").first().click();

    await expect(page.locator('input[value="DB_HOST"]')).not.toBeVisible();

    await page.getByRole("button", { name: "Save Changes" }).click();

    await page.getByRole("button", { name: "Edit Config" }).click();
    await expect(page.locator('input[value="DB_HOST"]')).not.toBeVisible();
});
