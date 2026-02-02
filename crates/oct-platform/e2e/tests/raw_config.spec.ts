import { expect, test } from "../fixtures";

test("raw config is displayed correctly", async ({ page }) => {
    const rawConfigBlock = page.locator("pre code");
    await expect(rawConfigBlock).toBeVisible();

    const text = await rawConfigBlock.innerText();

    // We expect [project] to be on one line, not split across lines
    expect(text).toContain("[project]");

    // The regex check: ensure we don't have [ followed by newline and then project
    expect(text).not.toMatch(/\[\s*\n\s*project/);
});
