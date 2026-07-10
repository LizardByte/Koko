<script module lang="ts">
  // UserAvatar stories — the circular avatar shown in the Rail + settings.
  // Fully props-driven: takes a BootstrapUser, renders an <img> if
  // profile_image_url is set, otherwise an initials fallback.
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import UserAvatar from './UserAvatar.svelte';
  import { mockUser } from '$lib/storybook/fixtures';

  const { Story } = defineMeta({
    title: 'Components/UserAvatar',
    tags: ['autodocs'],
    args: { preset: 'empty' },
    parameters: {
      docs: {
        description: {
          component:
            'Circular avatar: <img> when profile_image_url is set, otherwise the first initial of the username. Used in the Rail user card + settings.',
        },
      },
    },
  });

  const admin = mockUser();
  const regular = mockUser({ id: 2, username: 'viewer', admin: false });
  const withImage = mockUser({ id: 3, username: 'photogenic', profile_image_url: 'mock://avatar.jpg' });
  const shortName = mockUser({ id: 4, username: 'k', admin: false });
</script>

<Story name="Admin (No Image)" args={{ preset: 'empty' }} asChild>
  <div style="display:flex;gap:1rem;align-items:center;">
    <UserAvatar user={admin} />
    <span>{admin.username}</span>
  </div>
</Story>

<Story name="Regular User" args={{ preset: 'empty' }} asChild>
  <div style="display:flex;gap:1rem;align-items:center;">
    <UserAvatar user={regular} />
    <span>{regular.username}</span>
  </div>
</Story>

<Story name="With Image" args={{ preset: 'empty' }} asChild>
  <div style="display:flex;gap:1rem;align-items:center;">
    <UserAvatar user={withImage} />
    <span>{withImage.username}</span>
  </div>
</Story>

<Story name="Short Name" args={{ preset: 'empty' }} asChild>
  <div style="display:flex;gap:1rem;align-items:center;">
    <UserAvatar user={shortName} />
    <span>{shortName.username}</span>
  </div>
</Story>
