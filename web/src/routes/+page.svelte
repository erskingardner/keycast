<script lang="ts">
    import SignInMenu from "$lib/components/SignInMenu.svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import {
        ArrowUpRight,
        Key,
        SlidersHorizontal,
        PlugsConnected,
    } from "phosphor-svelte";
    const user = $derived(getCurrentUser()?.user);
</script>

<svelte:head
    ><title>Keycast — Your keys, on your terms</title><meta
        name="description"
        content="A personal home for your Nostr keys. Manage access, connect your apps, and keep control with Keycast."
    /></svelte:head
>
<div class="landing">
    <div class="landing-intro eyebrow">
        <span>Personal key infrastructure</span><span
            >Nostr / Remote signing</span
        >
    </div>
    <section class="landing-hero" aria-labelledby="welcome-title">
        <div>
            <h1 class="landing-title" id="welcome-title">
                Your keys.<br />Your server.<br /><span>Your terms.</span>
            </h1>
            <p class="landing-description">
                One home for your Nostr keys. Connect your apps, decide what
                they can do, and manage access from a single workspace.
            </p>
            <div class="flex items-center gap-4 mt-7">
                {#if user}<a href="/teams" class="button button-primary"
                        >Open workspace <ArrowUpRight size={16} /></a
                    >{:else}<SignInMenu />{/if}<span class="description"
                    >Bring your own signer.</span
                >
            </div>
        </div>
        <div class="overview-panel">
            <div class="overview-title">
                <span class="eyebrow">From key to connected app</span
                ><ArrowUpRight size={18} />
            </div>
            <div class="overview-step">
                <span class="eyebrow">01</span>
                <div>
                    <h2>A place for your keys</h2>
                    <p>
                        Organize keys into teams. Import from your trusted CLI
                        or use the browser when you need it.
                    </p>
                </div>
            </div>
            <div class="overview-step">
                <span class="eyebrow">02</span>
                <div>
                    <h2>Access with clear limits</h2>
                    <p>
                        Choose which events an app can sign and whether it can
                        encrypt or decrypt. Everything else is denied.
                    </p>
                </div>
            </div>
            <div class="overview-step">
                <span class="eyebrow">03</span>
                <div>
                    <h2>Connect. Review. Revoke.</h2>
                    <p>
                        Pair an app with a one-time invitation. Review its grant
                        and session count, or revoke access in place.
                    </p>
                </div>
            </div>
            <div class="px-5 py-3 border-t border-line flex justify-between">
                <span class="eyebrow">Self-hosted by design</span><span
                    class="badge">NIP-46</span
                >
            </div>
        </div>
    </section>
    <section class="landing-bottom" aria-label="Workspace capabilities">
        <article>
            <Key size={20} weight="light" />
            <h2>Keep the key. Share the capability.</h2>
            <p class="description">
                Connected apps request signatures. They do not receive the
                managed private key.
            </p>
        </article>
        <article>
            <SlidersHorizontal size={20} weight="light" />
            <h2>See the whole picture.</h2>
            <p class="description">
                Keys, policies, members, and recent activity together. Fewer
                pages between you and the decision.
            </p>
        </article>
        <article>
            <PlugsConnected size={20} weight="light" />
            <h2>Use the signer you trust.</h2>
            <p class="description">
                Sign in with a browser extension or remote signer. Approve
                management changes with your external key store.
            </p>
        </article>
    </section>
    <footer class="site-footer">
        <span>KEYCAST</span><span>Built for a small, self-hosted instance.</span
        >
    </footer>
</div>
