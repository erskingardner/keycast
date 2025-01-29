<script lang="ts">
import type { PolicyWithPermissions } from "$lib/types";
import { readablePermissionConfig } from "$lib/utils/permissions";
import { toTitleCase } from "$lib/utils/strings";

let {
    policy,
    hoverable = false,
}: { policy: PolicyWithPermissions; hoverable?: boolean } = $props();
</script>


<div class="card {hoverable ? 'hover-card' : ''}">
    <h3 class="text-lg font-semibold">{policy.policy.name}</h3>
    <ul class="">
        {#each policy.permissions as permission}
            <li class="text-sm text-gray-300">{toTitleCase(permission.identifier)}
                <ul class="list-disc list-inside ml-2">
                    {#each readablePermissionConfig(permission) as config}
                        <li class="text-xs text-gray-400">{config}</li>
                    {/each}
                </ul>
            </li>
        {/each}
    </ul>
</div>
