import { expect, test } from "@playwright/test";

test("loads design tokens and avoids horizontal overflow", async ({ page }) => {
  await page.goto("/");
  const root = page.locator(":root");
  await expect
    .poll(async () => root.evaluate((el) => getComputedStyle(el).getPropertyValue("--color-canvas").trim()))
    .not.toBe("");
  await expect
    .poll(async () => root.evaluate((el) => getComputedStyle(el).getPropertyValue("--space-4").trim()))
    .not.toBe("");
  await expect
    .poll(async () => root.evaluate((el) => getComputedStyle(el).getPropertyValue("--radius-lg").trim()))
    .not.toBe("");

  const topbar = page.locator(".topbar");
  await expect(topbar).toBeVisible();
  const padding = await topbar.evaluate((el) => getComputedStyle(el).padding);
  expect(padding).not.toBe("");

  const input = page.locator(".input").first();
  if (await input.count()) {
    const border = await input.evaluate((el) => getComputedStyle(el).borderStyle);
    const inputPadding = await input.evaluate((el) => getComputedStyle(el).padding);
    expect(border).not.toBe("none");
    expect(inputPadding).not.toBe("");
  }

  const sidebar = page.locator(".sidebar");
  await expect(sidebar).toBeVisible();
  const background = await sidebar.evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(background).not.toBe("");

  const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
  const clientWidth = await page.evaluate(() => document.documentElement.clientWidth);
  expect(scrollWidth).toBeLessThanOrEqual(clientWidth + 1);
});
