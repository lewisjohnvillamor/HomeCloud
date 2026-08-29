import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import FilesPage from "@/app/files/page";
import HomePage from "@/app/page";
import MorePage from "@/app/more/page";
import PhotosPage from "@/app/photos/page";
import SearchPage from "@/app/search/page";

describe("section pages", () => {
  it.each([
    ["Home", HomePage, "HomeCloud"],
    ["Files", FilesPage, "Files"],
    ["Photos", PhotosPage, "Photos"],
    ["Search", SearchPage, "Search"],
    ["More", MorePage, "More"],
  ])("%s renders one heading and an honest empty state", (_name, Page, heading) => {
    render(<Page />);

    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(heading);
    // Every section states what is missing rather than showing sample data.
    expect(screen.getByRole("heading", { level: 2 })).toBeInTheDocument();
  });
});
