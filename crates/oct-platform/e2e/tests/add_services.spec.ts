import { expect, test } from "../fixtures";

test("can add and edit multiple services", async ({ page }) => {
    await expect(page).toHaveTitle(/Project Config/);

    await page.getByRole("button", { name: "Edit Config" }).click();
    await expect(
        page.getByRole("heading", { name: "Edit Configuration" }),
    ).toBeVisible();

    await page.getByRole("button", { name: "Add Service" }).click();
    await expect(page.locator('input[value="service_1"]')).toBeVisible();

    // Wait slightly for potential HTMX swap
    await page.waitForTimeout(200);
    await page.getByRole("button", { name: "Add Service" }).click();
    await expect(page.locator('input[value="service_2"]')).toBeVisible();

    const service1Name = page.locator('input[name="services[0][name]"]');
    const service1Image = page.locator('input[name="services[0][image]"]');
    const service1Cpu = page.locator('input[name="services[0][cpus]"]');

    await service1Name.fill("frontend-app");
    await service1Image.fill("my-repo/frontend:v1");
    await service1Cpu.fill("500");

    const service2Name = page.locator('input[name="services[1][name]"]');
    const service2Image = page.locator('input[name="services[1][image]"]');
    const service2Mem = page.locator('input[name="services[1][memory]"]');

    await service2Name.fill("backend-api");
    await service2Image.fill("my-repo/backend:latest");
    await service2Mem.fill("1024");

    await page.getByRole("button", { name: "Save Changes" }).click();

    const servicesGrid = page.locator("#services-grid");
    await expect(
        servicesGrid.locator("h4", { hasText: "frontend-app" }),
    ).toBeVisible();
    await expect(
        servicesGrid
            .locator(".bg-surface")
            .filter({ hasText: "frontend-app" })
            .getByText("my-repo/frontend:v1"),
    ).toBeVisible();
    await expect(servicesGrid.getByText("500m")).toBeVisible();

    await expect(
        servicesGrid.locator("h4", { hasText: "backend-api" }),
    ).toBeVisible();
    await expect(
        servicesGrid
            .locator(".bg-surface")
            .filter({ hasText: "backend-api" })
            .getByText("my-repo/backend:latest"),
    ).toBeVisible();
    await expect(servicesGrid.getByText("1024 MB")).toBeVisible();
});
