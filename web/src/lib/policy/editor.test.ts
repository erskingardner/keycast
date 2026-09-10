import { describe, expect, test } from "bun:test";
import type { PolicyDocument } from "$lib/types";
import { CRYPTO_METHODS, POLICY_USE_CASES } from "./catalog";
import { buildDocument, canAddKind, createEditorState, cryptoScope, defaultKinds, disableKindsEverywhere, kindSources, searchUseCases, selectUseCase, setKinds, signingKinds, validateDocument } from "./editor";

const useCase = (id: string) => POLICY_USE_CASES.find(c => c.id === id)!;
const add = (id: string, state = createEditorState()) => selectUseCase(state, useCase(id), true);

describe("explicit policy editing", () => {
    test("starts denied and rejects an empty policy", () => {
        expect(signingKinds(createEditorState())).toEqual([]);
        expect(() => buildDocument(createEditorState())).toThrow("Choose at least one");
    });

    test("reload preserves every exact kind and crypto scope without guessing templates", () => {
        const document: PolicyDocument = { version: 1, capabilities: {
            sign_event: { allowed_kinds: [1, 1234, 30023, 31234, 65000] },
            nip44_encrypt: { recipient: "self_only" }, nip04_decrypt: { recipient: "any" },
        } };
        const state = createEditorState(document);
        expect(state.selected).toEqual({});
        expect(cryptoScope(state, "nip44_decrypt")).toBe("off");
        expect(buildDocument(state)).toEqual(document);
    });

    test("each catalog preset round trips without adding inferred permissions", () => {
        expect(new Set(POLICY_USE_CASES.map(c => c.id)).size).toBe(POLICY_USE_CASES.length);
        for (const c of POLICY_USE_CASES) {
            expect(c.items.flatMap(i => i.kinds).every(canAddKind)).toBe(true);
            const document = buildDocument(add(c.id));
            expect(buildDocument(createEditorState(document))).toEqual(document);
        }
    });

    test("shared kinds remain allowed until all their sources are removed", () => {
        let state = add("comments", add("gitcontrib"));
        state = setKinds(state, "comments", [1111], false);
        expect(signingKinds(state)).toContain(1111);
        expect(kindSources(state, 1111, "comments")).toEqual(["Git contributions"]);
        state = disableKindsEverywhere(state, [1111]);
        expect(signingKinds(state)).not.toContain(1111);
    });

    test("custom overlaps survive template removal and serialize only once", () => {
        let state = setKinds(add("notes"), "custom", [1, 65000], true);
        expect(signingKinds(state)).toEqual([1, 65000]);
        state = selectUseCase(state, useCase("notes"), false);
        expect(signingKinds(state)).toEqual([1, 65000]);
        state = disableKindsEverywhere(state, [1]);
        expect(signingKinds(state)).toEqual([65000]);
    });

    test("reselecting a selected template preserves its customized kinds", () => {
        const state = setKinds(add("longform"), "longform", [1234], false);
        expect(add("longform", state).selected.longform).not.toContain(1234);
    });

    test("draft crypto follows enabled capabilities, with legacy off by default", () => {
        let state = add("longform");
        expect(signingKinds(state)).toEqual([1234, 30023, 31234]);
        expect(cryptoScope(state, "nip44_encrypt")).toBe("self_only");
        expect(cryptoScope(state, "nip44_decrypt")).toBe("self_only");
        expect(cryptoScope(state, "nip04_encrypt")).toBe("off");
        state = setKinds(state, "longform", [1234, 31234], false);
        expect(cryptoScope(state, "nip44_encrypt")).toBe("off");
        state = setKinds(state, "longform", [30024], true);
        expect(cryptoScope(state, "nip04_encrypt")).toBe("self_only");
        expect(cryptoScope(state, "nip04_decrypt")).toBe("self_only");
    });

    test("NIP-17 uses identity seals and any-peer crypto, not unsigned rumors or temporary wraps", () => {
        const state = add("dm");
        expect(signingKinds(state)).toEqual([13, 10050]);
        expect(cryptoScope(state, "nip44_encrypt")).toBe("any");
        expect(cryptoScope(state, "nip44_decrypt")).toBe("any");
        expect(useCase("dm").transport?.some(t => t.kind === 1059)).toBe(true);
    });

    test("Marmot separates account signing from MLS and temporary transport keys", () => {
        const state = add("marmot");
        expect(signingKinds(state)).toEqual([13, 450, 10050, 30443]);
        expect(cryptoScope(state, "nip44_encrypt")).toBe("any");
        expect(cryptoScope(state, "nip44_decrypt")).toBe("any");
        for (const kind of [9, 443, 444, 445, 451, 452, 1059, 10051]) expect(defaultKinds(useCase("marmot"))).not.toContain(kind);
    });

    test("removing messaging narrows suggested crypto back to self-only drafts", () => {
        let state = add("dm", add("longform"));
        expect(cryptoScope(state, "nip44_decrypt")).toBe("any");
        state = selectUseCase(state, useCase("dm"), false);
        expect(cryptoScope(state, "nip44_decrypt")).toBe("self_only");
    });

    test("manual narrower or denied crypto overrides template suggestions", () => {
        const state = add("dm");
        state.overrides = { nip44_encrypt: "self_only", nip44_decrypt: "off" };
        expect(buildDocument(state).capabilities.nip44_encrypt).toEqual({ recipient: "self_only" });
        expect(buildDocument(state).capabilities.nip44_decrypt).toBeUndefined();
    });

    test("saved crypto survives template removal and can still be disabled", () => {
        let state = createEditorState({ version: 1, capabilities: { nip44_decrypt: { recipient: "self_only" } } });
        state = add("dm", state);
        expect(cryptoScope(state, "nip44_decrypt")).toBe("any");
        state = selectUseCase(state, useCase("dm"), false);
        expect(cryptoScope(state, "nip44_decrypt")).toBe("self_only");
        expect(cryptoScope(state, "nip44_encrypt")).toBe("off");
    });

    test("all four crypto methods work independently without signing", () => {
        for (const method of CRYPTO_METHODS) for (const recipient of ["any", "self_only"] as const) {
            const state = createEditorState();
            state.overrides[method] = recipient;
            expect(buildDocument(state)).toEqual({ version: 1, capabilities: { [method]: { recipient } } });
        }
    });

    test("search matches protocol, transport, capability, kind, and multiple words", () => {
        expect(searchUseCases("marmot").map(c => c.id)).toContain("marmot");
        expect(searchUseCases("nip 17").map(c => c.id)).toContain("dm");
        expect(searchUseCases("1059").map(c => c.id)).toEqual(expect.arrayContaining(["dm", "marmot", "giftwrap"]));
        expect(searchUseCases("long drafts").map(c => c.id)).toEqual(["longform"]);
        expect(searchUseCases("nonexistent-use-case")).toEqual([]);
    });

    test("custom kind boundary excludes private management", () => {
        expect(canAddKind(0)).toBe(true);
        expect(canAddKind(65535)).toBe(true);
        for (const kind of [-1, 65536, 1.5, NaN, 27236, 27237]) expect(canAddKind(kind)).toBe(false);
    });

    test("unknown or malformed saved policies fail closed", () => {
        const malformed = [null, [], {}, { version: 2, capabilities: { nip44_encrypt: { recipient: "any" } } },
            { version: 1, capabilities: {} }, { version: 1, capabilities: { future_method: {} } },
            { version: 1, capabilities: { sign_event: { allowed_kinds: [] } } },
            { version: 1, capabilities: { sign_event: { allowed_kinds: [1, 1] } } },
            { version: 1, capabilities: { sign_event: { allowed_kinds: ["1"] } } },
            { version: 1, capabilities: { sign_event: { allowed_kinds: [65536] } } },
            { version: 1, capabilities: { nip44_decrypt: { recipient: "unknown" } } },
            { version: 1, capabilities: { nip44_decrypt: { recipient: ["any"] } } },
            { version: 1, capabilities: { nip44_encrypt: { recipient: "any", peer: "extra" } } },
            { version: 1, capabilities: { nip44_encrypt: { recipient: "any" } }, extra: true },
        ];
        for (const value of malformed) expect(() => validateDocument(value)).toThrow("cannot be edited safely");
    });
});
