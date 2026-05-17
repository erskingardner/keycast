<script lang="ts">
import { page } from "$app/stores";
import { getCurrentUser } from "$lib/current_user.svelte";
import { hasAmberSignerSupport, hasNip07Extension } from "$lib/nostr";
import { signin, signout, type SigninMethod } from "$lib/utils/auth";
import { Key, SignIn, SignOut } from "phosphor-svelte";

const user = $derived(getCurrentUser()?.user);
const activePage = $derived($page.url.pathname);
let signInMenuOpen = $state(false);

async function startSignin(method: SigninMethod) {
    signInMenuOpen = false;
    await signin(method);
}
</script>


<div class="container flex flex-row items-center justify-between mb-12">
	<a href="/" class="flex flex-col items-start justify-start">
		<h1 class="text-3xl font-bold flex flex-row gap-1 items-center">
            <Key size="32" weight="fill" />
            Keycast
        </h1>
		<p class="hidden md:block text-gray-400">Secure remote signing for your team</p>
	</a>

    <nav class="flex flex-row items-center justify-start gap-4">
        {#if user}
            <a class="nav-link {activePage === '/teams' ? 'active' : ''} bordered" href="/teams">Teams</a>
            <button
                onclick={() => signout()}
                ontouchend={() => signout()}
                class="button button-secondary button-icon"
                role="menuitem"
                tabindex="-1"
                id="user-menu-item-1"
            >
                <SignOut size="20" />
                Sign out
            </button>
        {:else}
            <div class="relative">
                <button
                    onclick={() => {
                        signInMenuOpen = !signInMenuOpen;
                    }}
                    class="button button-primary button-icon"
                >
                    <SignIn size="20" />
                    Sign in
                </button>
                {#if signInMenuOpen}
                    <div class="absolute right-0 top-12 z-20 min-w-48 rounded-md bg-gray-800 p-2 text-sm shadow-lg ring-1 ring-gray-700">
                        {#if hasNip07Extension()}
                            <button class="menu-item" onclick={() => startSignin("extension")}>
                                Browser extension
                            </button>
                        {/if}
                        {#if hasAmberSignerSupport()}
                            <button class="menu-item" onclick={() => startSignin("amber")}>
                                Amber
                            </button>
                        {/if}
                        <button class="menu-item" onclick={() => startSignin("remote")}>
                            Remote signer
                        </button>
                    </div>
                {/if}
            </div>
        {/if}
    </nav>
</div>

<style>
    .menu-item {
        display: block;
        width: 100%;
        border-radius: 0.375rem;
        padding: 0.5rem 0.75rem;
        text-align: left;
        color: rgb(229 231 235);
    }

    .menu-item:hover {
        background: rgb(55 65 81);
        color: white;
    }
</style>
