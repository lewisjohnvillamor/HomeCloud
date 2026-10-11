import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ActionMenu, type MenuAction } from "@/components/ui/action-menu";

function actions(onSelect = vi.fn()): MenuAction[] {
  return [
    { key: "copy", label: "Copy", icon: "copy", onSelect: () => onSelect("copy") },
    { key: "rename", label: "Rename", icon: "rename", onSelect: () => onSelect("rename") },
    { key: "delete", label: "Delete", icon: "trash", danger: true, onSelect: () => onSelect("delete") },
  ];
}

describe("ActionMenu", () => {
  it("names what it acts on, so a list of More buttons is not ambiguous", () => {
    render(<ActionMenu name="notes.txt" actions={actions()} />);
    expect(screen.getByRole("button", { name: /More\s*actions for notes\.txt/ })).toBeInTheDocument();
  });

  it("is closed until opened, and says so", async () => {
    render(<ActionMenu name="notes.txt" actions={actions()} />);
    const trigger = screen.getByRole("button", { name: /More/ });

    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("menuitem", { name: "Delete" })).not.toBeInTheDocument();

    await userEvent.click(trigger);

    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("menu", { name: "Actions for notes.txt" })).toBeVisible();
  });

  it("moves focus into the menu and through it with the arrow keys", async () => {
    render(<ActionMenu name="notes.txt" actions={actions()} />);
    await userEvent.click(screen.getByRole("button", { name: /More/ }));

    expect(screen.getByRole("menuitem", { name: "Copy" })).toHaveFocus();

    await userEvent.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "Rename" })).toHaveFocus();

    await userEvent.keyboard("{ArrowUp}{ArrowUp}");
    expect(screen.getByRole("menuitem", { name: "Delete" })).toHaveFocus();

    await userEvent.keyboard("{Home}");
    expect(screen.getByRole("menuitem", { name: "Copy" })).toHaveFocus();
  });

  it("runs the chosen action and closes", async () => {
    const onSelect = vi.fn();
    render(<ActionMenu name="notes.txt" actions={actions(onSelect)} />);
    const trigger = screen.getByRole("button", { name: /More/ });

    await userEvent.click(trigger);
    await userEvent.click(screen.getByRole("menuitem", { name: "Rename" }));

    expect(onSelect).toHaveBeenCalledWith("rename");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("menuitem", { name: "Rename" })).not.toBeInTheDocument();
  });

  it("names a destructive action in words", async () => {
    render(<ActionMenu name="notes.txt" actions={actions()} />);
    await userEvent.click(screen.getByRole("button", { name: /More/ }));

    expect(screen.getByRole("menuitem", { name: "Delete" })).toHaveAttribute("data-danger", "true");
  });
});
