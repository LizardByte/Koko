<script lang="ts">
  // UserManagement — per-user edit rows + create-user form. Port of
  // renderUserManagement (../client-web/src/app/auth.ts:111-168).
  //
  // Profile-image upload: <input type="file"> → FileReader.readAsDataURL →
  // strip data-URL prefix → { mimeType, data_base64 } in the JSON body
  // (matching the backend's ProfileImageUploadForm: { mime_type, data_base64 }).
  // No multipart endpoint exists — the Rust server's update_user takes
  // Json<UpdateUserForm> with ProfileImageUploadForm as a nested struct.
  // Port of readProfileImageUpload (auth.ts:24-48).
  import Button from '../Button.svelte';
  import UserAvatar from '../UserAvatar.svelte';
  import { auth, ui } from '$lib/stores';
  import type { CreateUserRequest, UpdateUserRequest } from '$lib/api';

  // --- Profile image reading (readProfileImageUpload port) ---

  const ALLOWED_IMAGE_TYPES = new Set(['image/jpeg', 'image/png', 'image/webp', 'image/gif']);
  const MAX_IMAGE_BYTES = 2 * 1024 * 1024;

  type ProfileImageUpload = { mime_type: string; data_base64: string };

  async function readProfileImage(file: File): Promise<ProfileImageUpload> {
    if (!ALLOWED_IMAGE_TYPES.has(file.type)) {
      throw new Error('Profile image must be a JPEG, PNG, WebP, or GIF file.');
    }
    if (file.size > MAX_IMAGE_BYTES) {
      throw new Error('Profile image must be 2 MB or smaller.');
    }
    const dataUrl = await readFileAsDataUrl(file);
    const dataBase64 = dataUrl.split(',')[1] ?? '';
    if (!dataBase64) throw new Error('Failed to read profile image.');
    return { mime_type: file.type, data_base64: dataBase64 };
  }
  function readFileAsDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.addEventListener('load', () => resolve(typeof reader.result === 'string' ? reader.result : ''));
      reader.addEventListener('error', () => reject(new Error('Failed to read profile image.')));
      reader.readAsDataURL(file);
    });
  }

  // --- Per-user edit state ---

  // Editable copies keyed by user id. Each row maintains its own form state.
  type UserEdit = {
    username: string;
    birthday: string;
    metadataLanguages: string;
    admin: boolean;
    removeProfileImage: boolean;
    imageFile?: File;
    imagePreview?: string;
    saving: boolean;
  };

  const edits = $state<Record<number, UserEdit>>({});

  // Sync local edit state when users load/change.
  $effect(() => {
    for (const user of auth.users) {
      if (!edits[user.id]) {
        edits[user.id] = {
          username: user.username,
          birthday: user.birthday ?? '',
          metadataLanguages: (user.preferred_metadata_languages ?? ['en-US']).join(', '),
          admin: user.admin,
          removeProfileImage: false,
          saving: false,
        };
      }
    }
  });

  async function onImageSelect(userId: number, event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const edit = edits[userId];
    if (edit) {
      edit.imageFile = file;
      // Preview via object URL (no upload yet — uploaded on Save).
      edit.imagePreview = URL.createObjectURL(file);
      edit.removeProfileImage = false;
    }
  }

  async function saveUser(userId: number, event: SubmitEvent) {
    event.preventDefault();
    const edit = edits[userId];
    if (!edit) return;
    edit.saving = true;
    try {
      const request: UpdateUserRequest = {
        username: edit.username,
        admin: edit.admin,
        birthday: edit.birthday || undefined,
        preferred_metadata_languages: edit.metadataLanguages
          .split(',')
          .map((l) => l.trim())
          .filter(Boolean),
        remove_profile_image: edit.removeProfileImage || undefined,
      };
      if (edit.imageFile) {
        request.profile_image_upload = await readProfileImage(edit.imageFile);
      }
      await auth.updateUser(userId, request);
      // Clear the preview + file after successful save.
      if (edit.imagePreview) URL.revokeObjectURL(edit.imagePreview);
      edit.imageFile = undefined;
      edit.imagePreview = undefined;
      edit.removeProfileImage = false;
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to update user.');
    } finally {
      edit.saving = false;
    }
  }

  // --- Create-user form ---

  let createUsername = $state('');
  let createPassword = $state('');
  let createPin = $state('');
  let createBirthday = $state('');
  let createMetadataLanguages = $state('en-US');
  let createAdmin = $state(false);
  let createImageFile = $state<File | undefined>(undefined);
  let createImagePreview = $state<string | undefined>(undefined);
  let creating = $state(false);

  async function onCreateImageSelect(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    createImageFile = file;
    createImagePreview = URL.createObjectURL(file);
  }

  async function createUser(event: SubmitEvent) {
    event.preventDefault();
    creating = true;
    try {
      const request: CreateUserRequest = {
        username: createUsername,
        password: createPassword,
        pin: createPin || undefined,
        birthday: createBirthday || undefined,
        preferred_metadata_languages: createMetadataLanguages
          .split(',')
          .map((l) => l.trim())
          .filter(Boolean),
        admin: createAdmin,
      };
      if (createImageFile) {
        request.profile_image_upload = await readProfileImage(createImageFile);
      }
      await auth.createUser(request);
      // Reset form
      createUsername = '';
      createPassword = '';
      createPin = '';
      createBirthday = '';
      createMetadataLanguages = 'en-US';
      createAdmin = false;
      if (createImagePreview) URL.revokeObjectURL(createImagePreview);
      createImageFile = undefined;
      createImagePreview = undefined;
      ui.clearError();
    } catch (err) {
      ui.setError(err instanceof Error ? err.message : 'Failed to create user.');
    } finally {
      creating = false;
    }
  }
</script>

{#if auth.canManageUsers}
  <section class="settings-form user-management-form">
    <div class="section-heading">
      <h3>Users</h3>
    </div>
    <div class="user-list">
      {#if auth.users.length === 0}
        <div class="empty-state tight">No users found.</div>
      {:else}
        {#each auth.users as user (user.id)}
          {@const edit = edits[user.id]}
          {#if edit}
            <form class="provider-row user-edit-row" onsubmit={(e) => saveUser(user.id, e)}>
              {#if edit.imagePreview}
                <img class="edit-avatar" src={edit.imagePreview} alt="" style="width:48px;height:48px;border-radius:50%;object-fit:cover;" />
              {:else}
                <UserAvatar user={user} class="edit-avatar" />
              {/if}
              <div class="user-edit-fields">
                <label>Username<input bind:value={edit.username} required /></label>
                <label>Birthday<input type="date" bind:value={edit.birthday} /></label>
                <label>Profile image<input type="file" accept="image/png,image/jpeg,image/webp,image/gif" onchange={(e) => onImageSelect(user.id, e)} /></label>
                <label>Metadata languages<input bind:value={edit.metadataLanguages} placeholder="en-US, es-ES" /></label>
                <label class="checkbox-inline"><input type="checkbox" bind:checked={edit.admin} /> Administrator</label>
                <label class="checkbox-inline"><input type="checkbox" bind:checked={edit.removeProfileImage} /> Remove image</label>
              </div>
              <div class="provider-tags">
                <span class="tag {user.admin ? 'success' : ''}">{user.admin ? 'Admin' : 'User'}</span>
                <Button variant="secondary" label="Save" icon="save" type="submit" busy={edit.saving} />
              </div>
            </form>
          {/if}
        {/each}
      {/if}
    </div>
  </section>

  <form class="settings-form user-management-form" onsubmit={createUser}>
    <section>
      <div class="section-heading">
        <h3>Add user</h3>
      </div>
      <label>Username<input bind:value={createUsername} autocomplete="off" required /></label>
      <label>Password<input type="password" bind:value={createPassword} autocomplete="new-password" required /></label>
      <label>Optional PIN<input inputmode="numeric" pattern={'\\d{4,6}'} placeholder="1234" bind:value={createPin} /></label>
      <label>Birthday<input type="date" bind:value={createBirthday} /></label>
      <label>Profile image<input type="file" accept="image/png,image/jpeg,image/webp,image/gif" onchange={onCreateImageSelect} /></label>
      {#if createImagePreview}
        <img src={createImagePreview} alt="" style="width:48px;height:48px;border-radius:50%;object-fit:cover;" />
      {/if}
      <label>Metadata languages<input bind:value={createMetadataLanguages} placeholder="en-US, es-ES" /></label>
      <label class="checkbox-inline"><input type="checkbox" bind:checked={createAdmin} /> Administrator</label>
      <Button label="Create user" icon="user-plus" type="submit" busy={creating} />
    </section>
  </form>
{/if}
