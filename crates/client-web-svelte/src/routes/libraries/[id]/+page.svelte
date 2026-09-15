<script lang="ts">
  // /libraries/[id] — renders HomeContent scoped to a specific library.
  import { page } from '$app/state';
  import HomeContent from '$lib/components/HomeContent.svelte';

  const libraryId = $derived(Number(page.params.id));
</script>

<HomeContent libraryId={libraryId} />
