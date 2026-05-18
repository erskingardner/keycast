<script lang="ts">
import {
    createNostrConnectSigninSession,
    hasNip07Extension,
    isAmberSigninSupported,
    type NostrConnectSigninOptions,
    type NostrConnectSigninSession,
} from "$lib/nostr";
import {
    completeSignin,
    signin,
    type SigninMethod,
    type SigninOptions,
} from "$lib/utils/auth";
import {
    Browser,
    Check,
    Cloud,
    Copy,
    DeviceMobile,
    LinkSimple,
    SignIn,
    X,
} from "phosphor-svelte";

type BusyState = SigninMethod | "nostr-connect-link" | "amber" | null;
type ConnectSessionStartOptions = {
    autoOpen?: boolean;
    busyState: Exclude<BusyState, null>;
    signerKind?: NostrConnectSigninOptions["signerKind"];
};

let open = $state(false);
let bunkerUri = $state("");
let busy: BusyState = $state(null);
let extensionAvailable = $state(false);
let amberAvailable = $state(false);
let connectUri = $state("");
let connectError = $state<string | null>(null);
let connectCopied = $state(false);
let connectSession: NostrConnectSigninSession | null = null;
let connectController: AbortController | null = null;
const signinOptionClass =
    "flex w-full items-center gap-3 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-3 text-left text-gray-100 hover:border-indigo-400/50 hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-50";

$effect(() => {
    if (open) {
        extensionAvailable = hasNip07Extension();
        amberAvailable = isAmberSigninSupported();
    }
});

function openMenu() {
    open = true;
}

function closeMenu() {
    cancelConnectLink();
    open = false;
    bunkerUri = "";
    connectError = null;
    busy = null;
}

async function runSignin(method: SigninMethod, options: SigninOptions = {}) {
    busy = method;
    const user = await signin(method, options);
    busy = null;

    if (user) {
        closeMenu();
    }
}

async function submitBunker(event: SubmitEvent) {
    event.preventDefault();
    await runSignin("nip46-bunker", { bunkerUri });
}

async function startConnectLink() {
    await startConnectSession({ busyState: "nostr-connect-link" });
}

async function startAmberConnect() {
    await startConnectSession({
        autoOpen: true,
        busyState: "amber",
        signerKind: "amber",
    });
}

async function startConnectSession(options: ConnectSessionStartOptions) {
    cancelConnectLink();
    connectError = null;
    connectCopied = false;
    connectSession = createNostrConnectSigninSession({
        signerKind: options.signerKind,
    });
    connectUri = connectSession.uri;

    const controller = new AbortController();
    connectController = controller;
    busy = options.busyState;

    if (options.autoOpen) {
        openConnectUri(connectUri);
    }

    try {
        const user = await connectSession.waitForUser(controller.signal);
        const signedInUser = await completeSignin(user);
        if (signedInUser) {
            closeMenu();
        }
    } catch (error) {
        if (!controller.signal.aborted) {
            connectError =
                error instanceof Error ? error.message : "Unable to connect signer";
        }
    } finally {
        const shouldClearConnectState =
            connectController === controller ||
            (controller.signal.aborted && connectController === null);

        if (shouldClearConnectState) {
            busy = null;
            connectController = null;
        }
    }
}

function openConnectUri(uri: string) {
    window.location.href = uri;
}

function cancelConnectLink() {
    connectController?.abort();
    connectSession?.cancel();
    connectController = null;
    connectSession = null;
    connectUri = "";
    connectError = null;
}

async function copyConnectUri() {
    if (!connectUri) return;

    await navigator.clipboard.writeText(connectUri);
    connectCopied = true;
    setTimeout(() => {
        connectCopied = false;
    }, 1500);
}
</script>

<button type="button" onclick={openMenu} class="button button-primary button-icon">
    <SignIn size="20" />
    Sign in
</button>

{#if open}
    <div class="fixed inset-0 z-50 flex items-start justify-center bg-black/60 px-4 py-8 md:items-center">
        <div
            class="w-full max-w-lg rounded-lg border border-white/15 bg-gray-900 shadow-2xl shadow-black/40"
            role="dialog"
            aria-modal="true"
            aria-labelledby="signin-title"
        >
            <div class="flex items-center justify-between border-b border-white/10 px-5 py-4">
                <div>
                    <h2 id="signin-title" class="text-lg font-semibold">Sign in</h2>
                    <p class="text-sm text-gray-400">Choose a signer</p>
                </div>
                <button
                    type="button"
                    onclick={closeMenu}
                    class="rounded-md p-1 text-gray-400 hover:bg-white/10 hover:text-white"
                    aria-label="Close sign-in options"
                >
                    <X size="20" />
                </button>
            </div>

            <div class="flex flex-col gap-3 p-5">
                <button
                    type="button"
                    onclick={() => runSignin("extension")}
                    disabled={busy !== null}
                    class={signinOptionClass}
                >
                    <Browser size="24" />
                    <span class="min-w-0 flex-1">
                        <span class="block font-medium">Browser extension</span>
                        <span class="block text-xs text-gray-400">
                            {extensionAvailable ? "NIP-07 detected" : "NIP-07 extension"}
                        </span>
                    </span>
                    <span class="text-xs text-gray-500">
                        {busy === "extension" ? "Opening" : "NIP-07"}
                    </span>
                </button>

                <div class="rounded-lg border border-white/10 bg-white/[0.03] p-3">
                    <div class="mb-3 flex items-center gap-3">
                        <Cloud size="24" class="text-indigo-300" />
                        <div>
                            <h3 class="font-medium">Remote signer</h3>
                            <p class="text-xs text-gray-400">Nostr Connect</p>
                        </div>
                    </div>

                    <form class="flex flex-col gap-2 sm:flex-row" onsubmit={submitBunker}>
                        <input
                            class="min-w-0 flex-1"
                            type="text"
                            bind:value={bunkerUri}
                            placeholder="bunker://..."
                            autocapitalize="off"
                            autocomplete="off"
                            spellcheck="false"
                        />
                        <button
                            type="submit"
                            class="button button-primary"
                            disabled={!bunkerUri.trim() || busy !== null}
                        >
                            {busy === "nip46-bunker" ? "Connecting" : "Connect"}
                        </button>
                    </form>

                    <div class="mt-3 flex flex-wrap items-center gap-2">
                        {#if connectUri}
                            <a href={connectUri} class="button button-secondary button-icon">
                                <LinkSimple size="18" />
                                Open
                            </a>
                            <button
                                type="button"
                                class="button button-secondary button-icon"
                                onclick={copyConnectUri}
                            >
                                {#if connectCopied}
                                    <Check size="18" />
                                {:else}
                                    <Copy size="18" />
                                {/if}
                                Copy
                            </button>
                            <button
                                type="button"
                                class="button button-secondary"
                                onclick={cancelConnectLink}
                            >
                                Cancel
                            </button>
                        {:else}
                            <button
                                type="button"
                                class="button button-secondary button-icon"
                                onclick={startConnectLink}
                                disabled={busy !== null}
                            >
                                <LinkSimple size="18" />
                                Create connect link
                            </button>
                        {/if}
                    </div>

                    {#if connectUri}
                        <input
                            class="mt-3 w-full font-mono text-xs"
                            type="text"
                            readonly
                            value={connectUri}
                            onclick={(event) => event.currentTarget.select()}
                        />
                    {/if}

                    {#if connectError}
                        <p class="mt-2 text-sm text-red-300">{connectError}</p>
                    {/if}
                </div>

                <button
                    type="button"
                    onclick={startAmberConnect}
                    disabled={busy !== null || !amberAvailable}
                    class={signinOptionClass}
                >
                    <DeviceMobile size="24" />
                    <span class="min-w-0 flex-1">
                        <span class="block font-medium">Connect with Amber</span>
                        <span class="block text-xs text-gray-400">
                            {amberAvailable ? "Nostr Connect ready" : "Android signer"}
                        </span>
                    </span>
                    <span class="text-xs text-gray-500">
                        {busy === "amber" ? "Opening" : "NIP-46"}
                    </span>
                </button>
            </div>
        </div>
    </div>
{/if}
