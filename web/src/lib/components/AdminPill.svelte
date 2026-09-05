<script lang="ts">
    import type { User } from "$lib/types";

    let { user }: { user: User } = $props();
</script>

<span class="badge" class:badge-neutral={user.role !== "admin"}
    >{user.role === "admin" ? "Admin" : "Member"}</span
>
