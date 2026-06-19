// Auth store — replaces the vanilla client's `state.bootstrap` + token-in-
// localStorage pattern. Implemented with a Svelte 5 class-based rune so any
// component can reactively read the current user via `auth.user`.
import {
  getAppBootstrap,
  loginUser,
  getStoredAuthToken,
  setStoredAuthToken,
  clearStoredAuthToken,
  type AppBootstrapResponse,
  type LoginRequest,
} from './api';

class AuthStore {
  bootstrap = $state<AppBootstrapResponse | undefined>(undefined);
  loading = $state(true);

  get isLoggedIn(): boolean {
    return Boolean(this.bootstrap?.current_user);
  }

  get currentUser() {
    return this.bootstrap?.current_user;
  }

  get requiresSetup(): boolean {
    return Boolean(this.bootstrap && !this.bootstrap.has_users);
  }

  async init() {
    this.loading = true;
    try {
      this.bootstrap = await getAppBootstrap();
    } finally {
      this.loading = false;
    }
  }

  async login(request: LoginRequest): Promise<void> {
    const { token } = await loginUser(request);
    setStoredAuthToken(token);
    await this.init();
  }

  logout() {
    clearStoredAuthToken();
    if (this.bootstrap) {
      this.bootstrap = { ...this.bootstrap, current_user: undefined };
    }
  }

  hasToken(): boolean {
    return getStoredAuthToken() !== null;
  }
}

export const auth = new AuthStore();
