<script lang="ts">
    import { page } from "$app/stores";
    import SignInMenu from "$lib/components/SignInMenu.svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { signout } from "$lib/utils/auth";
    import { Key, SignOut } from "phosphor-svelte";
    const user = $derived(getCurrentUser()?.user);
</script>

<header class="app-header">
    <div class="header-inner">
        <a href={user ? "/teams" : "/"} class="brand" aria-label="Keycast home"
            ><span class="brand-mark"><Key size={23} weight="bold" /></span
            >keycast</a
        >
        <nav class="header-nav" aria-label="Main navigation">
            {#if user}
                <a
                    class="nav-link"
                    class:active={$page.url.pathname.startsWith("/teams")}
                    href="/teams"
                    aria-current={$page.url.pathname.startsWith("/teams")
                        ? "page"
                        : undefined}>Workspace</a
                >
                <a
                    class="nav-link"
                    class:active={$page.url.pathname === "/status"}
                    href="/status"
                    aria-current={$page.url.pathname === "/status"
                        ? "page"
                        : undefined}>Instance</a
                >
                <button
                    onclick={signout}
                    class="button button-quiet"
                    aria-label="Sign out"
                    ><SignOut size={18} /><span class="hidden sm:inline"
                        >Sign out</span
                    ></button
                >
            {:else}
                <span class="eyebrow hidden sm:block"
                    >Your keys. Your infrastructure.</span
                >
                <SignInMenu />
            {/if}
        </nav>
    </div>
</header>
