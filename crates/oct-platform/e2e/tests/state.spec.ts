import { expect, test } from "../fixtures";

test("state page displays graph and json", async ({ page, server }) => {
    await page.goto(`${server.url}/projects/default/state`);

    await expect(page).toHaveTitle(/Infrastructure State/);
    await expect(
        page.getByRole("heading", { name: "Infrastructure State" }),
    ).toBeVisible();

    await expect(page.locator(".mermaid")).toBeVisible();
    await expect(page.locator(".mermaid")).toContainText("Root");

    await expect(page.locator("code")).toBeVisible();
});
