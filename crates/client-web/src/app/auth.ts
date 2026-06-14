/** Handles authentication state helpers and authentication screen markup. */
import kokoLogoUrl from '../../../../assets/Koko.svg';
import type { BootstrapUser, ProfileImageUploadRequest } from '../api';
import { escapeHtml } from './format';
import { state } from './state';
import { renderButtonContent, renderUserAvatar } from './ui';

export function currentUser(): BootstrapUser | undefined {
  return state.bootstrap?.current_user;
}

export function requiresSetup(): boolean {
  return state.bootstrap?.has_users === false;
}

export function requiresLogin(): boolean {
  return state.bootstrap?.has_users === true && !currentUser();
}

export function canManageUsers(): boolean {
  return currentUser()?.admin ?? false;
}

export async function readProfileImageUpload(formData: FormData): Promise<ProfileImageUploadRequest | undefined> {
  const value = formData.get('profile_image_file');
  if (!(value instanceof File) || value.size === 0) {
    return undefined;
  }

  const allowedTypes = new Set(['image/jpeg', 'image/png', 'image/webp', 'image/gif']);
  if (!allowedTypes.has(value.type)) {
    throw new Error('Profile image must be a JPEG, PNG, WebP, or GIF file.');
  }
  if (value.size > 2 * 1024 * 1024) {
    throw new Error('Profile image must be 2 MB or smaller.');
  }

  const dataUrl = await readFileAsDataUrl(value);
  const dataBase64 = dataUrl.split(',')[1] ?? '';
  if (!dataBase64) {
    throw new Error('Failed to read profile image.');
  }

  return {
    mime_type: value.type,
    data_base64: dataBase64,
  };
}

export function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener('load', () => resolve(typeof reader.result === 'string' ? reader.result : ''));
    reader.addEventListener('error', () => reject(new Error('Failed to read profile image.')));
    reader.readAsDataURL(file);
  });
}

export function renderAuthShell(title: string, description: string, content: string): string {
  return `
    <div class="auth-shell">
      <section class="auth-panel panel">
        <div class="auth-header">
          <div class="brand-mark logo-brand-mark"><img class="brand-logo" src="${escapeHtml(kokoLogoUrl)}" alt="" /></div>
          <div>
            <h1>Koko</h1>
            <p class="muted">${escapeHtml(description)}</p>
          </div>
        </div>
        <div class="auth-copy">
          <h2>${escapeHtml(title)}</h2>
        </div>
        ${state.error ? `<section class="error-panel auth-error-panel">${escapeHtml(state.error)}</section>` : ''}
        ${content}
      </section>
    </div>
  `;
}

export function renderWelcomeScreen(): string {
  return renderAuthShell(
    'Create the first admin user',
    'Koko needs one administrator account before the media library can be used.',
    `
      <form id="welcome-user-form" class="auth-form">
        <label>Username<input name="username" autocomplete="username" required /></label>
        <label>Password<input name="password" type="password" autocomplete="new-password" required /></label>
        <label>Optional PIN<input name="pin" inputmode="numeric" pattern="[0-9]{4,6}" placeholder="1234" /></label>
        <label>Birthday<input name="birthday" type="date" /></label>
        <label>Profile image<input name="profile_image_file" type="file" accept="image/png,image/jpeg,image/webp,image/gif" /></label>
        <button type="submit">${renderButtonContent('Create admin account', 'user-plus')}</button>
      </form>
    `,
  );
}

export function renderLoginScreen(): string {
  return renderAuthShell(
    'Sign in',
    'Sign in with a Koko account to browse media and keep watch progress per user.',
    `
      <form id="login-form" class="auth-form">
        <label>Username<input name="username" autocomplete="username" required /></label>
        <label>Password<input name="password" type="password" autocomplete="current-password" required /></label>
        <button type="submit">${renderButtonContent('Sign in', 'log-in')}</button>
      </form>
    `,
  );
}

export function renderUserManagement(): string {
  if (!canManageUsers()) {
    return '';
  }

  let userListMarkup = '<div class="empty-state tight">No users found.</div>';
  if (state.users.length) {
    userListMarkup = state.users.map((user) => {
      const adminChecked = user.admin ? 'checked' : '';
      const adminTagClass = user.admin ? 'success' : '';
      const adminTagLabel = user.admin ? 'Admin' : 'User';
      return `
              <form class="provider-row user-edit-row" data-update-user-id="${user.id}">
                ${renderUserAvatar(user, 'edit-avatar')}
                <div class="user-edit-fields">
                  <label>Username<input name="username" value="${escapeHtml(user.username)}" required /></label>
                  <label>Birthday<input name="birthday" type="date" value="${escapeHtml(user.birthday ?? '')}" /></label>
                  <label>Profile image<input name="profile_image_file" type="file" accept="image/png,image/jpeg,image/webp,image/gif" /></label>
                  <label>Metadata languages<input name="preferred_metadata_languages" value="${escapeHtml((user.preferred_metadata_languages ?? ['en-US']).join(', '))}" placeholder="en-US, es-ES" /></label>
                  <label class="checkbox-inline"><input name="admin" type="checkbox" ${adminChecked} /> Administrator</label>
                  <label class="checkbox-inline"><input name="remove_profile_image" type="checkbox" /> Remove image</label>
                </div>
                <div class="provider-tags">
                  <span class="tag ${adminTagClass}">${adminTagLabel}</span>
                  <button type="submit" class="secondary-button">${renderButtonContent('Save', 'save')}</button>
                </div>
              </form>
            `;
    }).join('');
  }

  return `
    <section class="settings-form user-management-form">
      <div class="section-heading">
        <h3>Users</h3>
      </div>
      <div class="user-list">
        ${userListMarkup}
      </div>
    </section>

    <form id="create-user-form" class="settings-form user-management-form">
      <section>
        <div class="section-heading">
          <h3>Add user</h3>
        </div>
        <label>Username<input name="username" autocomplete="off" required /></label>
        <label>Password<input name="password" type="password" autocomplete="new-password" required /></label>
        <label>Optional PIN<input name="pin" inputmode="numeric" pattern="[0-9]{4,6}" placeholder="1234" /></label>
        <label>Birthday<input name="birthday" type="date" /></label>
        <label>Profile image<input name="profile_image_file" type="file" accept="image/png,image/jpeg,image/webp,image/gif" /></label>
        <label>Metadata languages<input name="preferred_metadata_languages" value="en-US" placeholder="en-US, es-ES" /></label>
        <label class="checkbox-inline"><input name="admin" type="checkbox" /> Administrator</label>
        <button type="submit">${renderButtonContent('Create user', 'user-plus')}</button>
      </section>
    </form>
  `;
}
