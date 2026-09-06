<script lang="ts">
    import SignInMenu from "$lib/components/SignInMenu.svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { ArrowRight } from "phosphor-svelte";
    const user = $derived(getCurrentUser()?.user);
</script>

<svelte:head
    ><title>Keycast — Your keys, under your control</title><meta
        name="description"
        content="Self-hosted Nostr signing. Manage your keys, define app permissions, and control access from one workspace."
    /></svelte:head
>
<div class="landing">
    <div class="landing-intro eyebrow">
        <span>key management, on your terms</span><span>nostr / nip-46</span>
    </div>
    <section class="landing-hero" aria-labelledby="welcome-title">
        <div>
            <p class="landing-prompt">
                <span aria-hidden="true">&gt; </span>your keys, your
                infrastructure
            </p>
            <h1 class="landing-title" id="welcome-title">
                Your keys.<br /><span>Under your control.</span>
            </h1>
            <p class="landing-description">
                A quiet home for your Nostr keys. Connect your apps, set their
                permissions, and manage access on your own server.
            </p>
            <div class="flex flex-wrap items-center gap-4 mt-7">
                {#if user}<a href="/teams" class="button button-primary"
                        >Open workspace <ArrowRight size={15} /></a
                    >{:else}<SignInMenu />{/if}
                <span class="description">Use your external signer.</span>
            </div>
        </div>
        <div class="overview-panel">
            <h2 class="overview-title">one workspace / three essentials</h2>
            <dl>
                <div class="overview-step">
                    <dt>01 / keys</dt>
                    <dd>
                        Organize identities into teams. Import from the trusted
                        CLI or your browser.
                    </dd>
                </div>
                <div class="overview-step">
                    <dt>02 / policy</dt>
                    <dd>
                        Allow specific signing and encryption capabilities.
                        Everything else is denied.
                    </dd>
                </div>
                <div class="overview-step">
                    <dt>03 / access</dt>
                    <dd>
                        Connect apps with one-time invitations. Review sessions
                        and revoke grants in place.
                    </dd>
                </div>
            </dl>
        </div>
    </section>
    <section class="landing-notes" aria-label="How access works">
        <article>
            <h2>keys stay with the signer</h2>
            <p>
                Apps request signatures through NIP-46. They do not receive the
                managed private key.
            </p>
        </article>
        <article>
            <h2>you approve management changes</h2>
            <p>
                Sign in with a browser extension or remote signer. Keep
                management approvals in your external key store.
            </p>
        </article>
    </section>
    <footer class="site-footer">
        <span>keycast</span><span>personal infrastructure. open protocol.</span>
    </footer>
</div>
