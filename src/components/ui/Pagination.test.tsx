// Pagination — numeric pager with prev/next + ellipsis around the current page.

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Pagination } from "./Pagination";

describe("Pagination", () => {
  it("renders nothing when there is at most one page", () => {
    const { container } = render(
      <Pagination page={0} totalPages={1} onChange={() => undefined} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("labels buttons 1-indexed even though `page` is 0-indexed", () => {
    render(<Pagination page={0} totalPages={3} onChange={() => undefined} />);
    expect(screen.getByRole("button", { name: "Go to page 1" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Go to page 2" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Go to page 3" })).toBeDefined();
  });

  it("marks the current page with aria-current=page", () => {
    render(<Pagination page={1} totalPages={3} onChange={() => undefined} />);
    const current = screen.getByRole("button", { name: "Go to page 2" });
    expect(current.getAttribute("aria-current")).toBe("page");
    const other = screen.getByRole("button", { name: "Go to page 1" });
    expect(other.getAttribute("aria-current")).toBeNull();
  });

  it("disables Prev on the first page and Next on the last page", () => {
    const { rerender } = render(
      <Pagination page={0} totalPages={3} onChange={() => undefined} />,
    );
    expect(screen.getByRole("button", { name: "Previous page" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next page" })).not.toBeDisabled();

    rerender(<Pagination page={2} totalPages={3} onChange={() => undefined} />);
    expect(screen.getByRole("button", { name: "Previous page" })).not.toBeDisabled();
    expect(screen.getByRole("button", { name: "Next page" })).toBeDisabled();
  });

  it("fires onChange with the 0-indexed page on click", () => {
    const onChange = vi.fn();
    render(<Pagination page={0} totalPages={3} onChange={onChange} />);

    fireEvent.click(screen.getByRole("button", { name: "Go to page 2" }));
    expect(onChange).toHaveBeenCalledWith(1);

    fireEvent.click(screen.getByRole("button", { name: "Next page" }));
    expect(onChange).toHaveBeenLastCalledWith(1);
  });

  it("collapses to ellipsis when total > 7", () => {
    render(<Pagination page={5} totalPages={20} onChange={() => undefined} />);
    // The visible sequence for page 5 of 20 is [1, …, 4, 5, 6, …, 20].
    const ellipses = screen.getAllByText("…");
    expect(ellipses.length).toBe(2);
    // Page 5 / page 6 should be visible.
    expect(screen.getByRole("button", { name: "Go to page 5" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Go to page 6" })).toBeDefined();
  });
});