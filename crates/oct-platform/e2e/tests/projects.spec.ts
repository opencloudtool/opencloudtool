import { expect, test } from "../fixtures";

test("workspace mode allows creating and switching projects", async ({
    page,
    workspaceServer,
}) => {
    // Navigate to dashboard
    await page.goto(`${workspaceServer.url}/projects`);

    // Create default project
    await page.fill('input[name="name"]', "default");
    await page.click('button:has-text("Create")');

    await page.click('button[title="Back to Projects"]');

    await expect(page.locator("h2")).toContainText("Projects");

    await expect(page.locator(".grid").getByText("default")).toBeVisible();

    await page.fill('input[name="name"]', "my-app");
    await page.click('button:has-text("Create")');

    await expect(page.locator("h2")).toContainText("my-app");

    await page.click('button[title="Back to Projects"]');
    await expect(page.locator("h2")).toContainText("Projects");

    await page.locator(".grid > div").filter({ hasText: "default" }).click();

    await expect(page.locator("h2")).toContainText("default");
});
