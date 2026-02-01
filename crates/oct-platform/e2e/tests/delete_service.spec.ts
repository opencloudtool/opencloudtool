import { expect, test } from "../fixtures";

test("can delete a service", async ({ page }) => {
    await page.getByRole("button", { name: "Edit Config" }).click();

    await page.getByRole("button", { name: "Add Service" }).click();
    await expect(page.locator('input[value="service_1"]')).toBeVisible();

    page.on("dialog", (dialog) => dialog.accept());

    const firstServiceCard = page
        .locator(".bg-surface")
        .filter({ hasText: "#1" });
    await expect(firstServiceCard).toBeVisible();

    await firstServiceCard.locator('button[title="Remove Service"]').click();

    await expect(firstServiceCard).not.toBeVisible();

    await page.getByRole("button", { name: "Save Changes" }).click();

    await expect(page.getByText("service_1")).not.toBeVisible();
});
