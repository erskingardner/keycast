import { describe, expect, test } from "bun:test";
import { safeRemoteImageUrl } from "./image_url";

describe("remote image URL safety", () => {
    test("allows absolute HTTP(S) image URLs and normalizes whitespace", () => {
        expect(safeRemoteImageUrl(" https://example.com/avatar.png ")).toBe(
            "https://example.com/avatar.png",
        );
        expect(safeRemoteImageUrl("http://example.com/banner.jpg")).toBe(
            "http://example.com/banner.jpg",
        );
    });

    test("rejects non-remote and script-bearing image URL values", () => {
        expect(safeRemoteImageUrl("javascript:alert(1)")).toBeNull();
        expect(safeRemoteImageUrl("data:image/svg+xml,<svg></svg>")).toBeNull();
        expect(safeRemoteImageUrl("/local-avatar.png")).toBeNull();
        expect(safeRemoteImageUrl("")).toBeNull();
        expect(safeRemoteImageUrl(null)).toBeNull();
    });
});
