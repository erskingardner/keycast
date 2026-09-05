import { describe, expect, test } from "bun:test";
import { managementDescription, MANAGEMENT_KIND } from "./management";

describe("external management approval contents", () => {
    test("is separate from ordinary NIP-98 and shows policy changes", () => {
        expect(MANAGEMENT_KIND).toBe(27236);
        const body = '{"name":"Notes","document":{"allowed_kinds":[1]}}';
        expect(managementDescription("PUT", "/teams/1/policies/2", body)).toBe(body);
    });
    test("never copies a private import into the approval event", () => {
        expect(managementDescription("POST", "/teams/1/keys", '{"name":"Personal","secret_key":"private-import-material"}')).toBe("Import private key named Personal");
    });
});
