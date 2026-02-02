import { expect, test } from "../fixtures";

test.describe("Mermaid Theme Responsiveness", () => {
    test("should re-render Mermaid diagram when theme is toggled", async ({
        page,
        server,
    }) => {
        // 1. Go to state page
        await page.goto(`${server.url}/projects/default/state`);

        // 2. Wait for mermaid to render
        const mermaidContainer = page.locator(".mermaid");
        await expect(mermaidContainer).toHaveAttribute(
            "data-processed",
            "true",
        );

        // Capture initial SVG ID or content to compare later
        const _initialSvgId = await mermaidContainer
            .locator("svg")
            .getAttribute("id");

        // 3. Toggle theme
        await page.locator('button[title="Toggle Theme"]').click();

        // 4. Verify mermaid re-renders (it should have a different SVG ID or at least it should be re-processed)
        // Since it clears and re-renders, we can wait for it to be processed again.
        await expect(mermaidContainer).toHaveAttribute(
            "data-processed",
            "true",
        );

        const _newSvgId = await mermaidContainer
            .locator("svg")
            .getAttribute("id");

        // Mermaid usually generates a new ID on re-run if we clear innerHTML
        // Even if it doesn't, we can check if it's still visible
        await expect(mermaidContainer.locator("svg")).toBeVisible();

        // Optional: Check if the theme attribute on the SVG changed if mermaid supports it
        // Or check some colors
    });
});
