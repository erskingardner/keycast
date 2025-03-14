<script lang="ts">
import { page } from "$app/stores";
import { getCurrentUser } from "$lib/current_user.svelte";
import ndk from "$lib/ndk.svelte";
import { SigninMethod, signin, signout } from "$lib/utils/auth";
import { Key, SignIn, SignOut } from "phosphor-svelte";

const user = $derived(getCurrentUser()?.user);
const activePage = $derived($page.url.pathname);
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
                onclick={() => signout(ndk)}
                ontouchend={() => signout(ndk)}
                class="button button-secondary button-icon"
                role="menuitem"
                tabindex="-1"
                id="user-menu-item-1"
            >
                <SignOut size="20" />
                Sign out
            </button>
        {:else}
            <button
                onclick={() => signin(ndk, undefined, SigninMethod.Nip07)}
                class="button button-primary button-icon"
            >
                <SignIn size="20" />
                Sign in
            </button>
        {/if}
    </nav>
</div>
