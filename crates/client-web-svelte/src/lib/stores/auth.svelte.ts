// Auth store — replaces the bootstrap + token pattern from app/auth.ts and
// app.ts. Exposes reactive `currentUser`, `isLoggedIn`, `requiresSetup`,
// `requiresLogin`, `canManageUsers` via `$derived`.
import {
  getAppBootstrap,
  loginUser,
  createUser,
  getUsers,
  updateUser,
  clearStoredAuthToken,
  setStoredAuthToken,
  type AppBootstrapResponse,
  type BootstrapUser,
  type LoginRequest,
  type CreateUserRequest,
  type UpdateUserRequest,
} from '../api';

class AuthStore {
  bootstrap = $state<AppBootstrapResponse | undefined>(undefined);
  users = $state<BootstrapUser[]>([]);
  loading = $state(true);

  get isLoggedIn(): boolean {
    return Boolean(this.bootstrap?.current_user);
  }

  get currentUser(): BootstrapUser | undefined {
    return this.bootstrap?.current_user;
  }

  get requiresSetup(): boolean {
    return this.bootstrap?.has_users === false;
  }

  get requiresLogin(): boolean {
    return this.bootstrap?.has_users === true && !this.isLoggedIn;
  }

  get canManageUsers(): boolean {
    return this.currentUser?.admin ?? false;
  }

  async init() {
    this.loading = true;
    try {
      this.bootstrap = await getAppBootstrap();
      if (this.canManageUsers) {
        this.users = await getUsers();
      }
    } finally {
      this.loading = false;
    }
  }

  async login(request: LoginRequest): Promise<void> {
    const { token } = await loginUser(request);
    setStoredAuthToken(token);
    await this.init();
  }

  async createUser(request: CreateUserRequest): Promise<void> {
    const token = await createUser(request);
    setStoredAuthToken(token);
    await this.init();
  }

  async updateUser(userId: number, request: UpdateUserRequest): Promise<void> {
    await updateUser(userId, request);
    this.users = await getUsers();
  }

  logout() {
    clearStoredAuthToken();
    if (this.bootstrap) {
      this.bootstrap = { ...this.bootstrap, current_user: undefined };
    }
  }
}

export const auth = new AuthStore();
