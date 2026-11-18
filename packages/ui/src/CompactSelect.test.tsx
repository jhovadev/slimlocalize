import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { CompactSelect } from "./CompactSelect";

describe("CompactSelect", () => {
  it("shows a custom listbox and chooses by keyboard", () => {
    const onChange = vi.fn();
    render(() => (
      <CompactSelect
        ariaLabel="Language"
        value="en"
        onChange={onChange}
        options={[
          { value: "en", label: "English" },
          { value: "es", label: "Spanish" },
        ]}
      />
    ));
    const trigger = screen.getByRole("combobox", { name: "Language" });
    fireEvent.click(trigger);
    expect(
      screen.getByRole("listbox", { name: "Language" }),
    ).toBeInTheDocument();
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(trigger, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("es");
  });
});
