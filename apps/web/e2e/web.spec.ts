import { expect, test } from "@playwright/test";

test("creates a workspace and persists a translation", async ({ page }) => {
  await page.goto("/");
  await expect(
    page.getByText("Your translations, on your machine."),
  ).toBeVisible();

  await page.getByLabel("Workspace name").fill("E2E Workspace");
  await page.getByRole("button", { name: "Create workspace" }).click();
  await expect(page.getByText("No projects in this workspace")).toBeVisible();

  await page.getByLabel("Project name").first().fill("E2E Project");
  await page.getByRole("button", { name: "Create project" }).first().click();
  await expect(page.getByText("Add your first locale")).toBeVisible();

  await page.getByLabel("Locale code").fill("en-US");
  await page.getByRole("button", { name: "Add locale" }).click();
  await expect(page.getByRole("button", { name: "New key" })).toBeVisible();

  await page.getByRole("button", { name: "New key" }).click();
  await page.getByLabel("Key").fill("welcome");
  await page.getByLabel("Value").fill("Welcome {name}");
  await page.getByRole("button", { name: "Save translation" }).click();
  await expect(page.getByText("common/welcome")).toBeVisible();

  await page.reload();
  await expect(page.getByText("common/welcome")).toBeVisible();
});
