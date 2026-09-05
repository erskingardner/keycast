<script lang="ts">
import type { User } from "$lib/types";

let { user }: { user: User } = $props();
</script>

<div class="pill {user.role === 'admin' ? 'bg-purple-500' : 'bg-indigo-500'}">
  <span class="text-white">{user.role === "admin" ? "Admin" : "Member"}</span>
</div>
