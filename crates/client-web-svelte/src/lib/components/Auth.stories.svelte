<script module>
  // Auth screen stories. LoginScreen reads the auth/ui stores (no props) and
  // only renders when bootstrap shows requiresLogin; WelcomeScreen renders
  // when requiresSetup. AuthShell is props-driven (title/description/children).
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import LoginScreen from './LoginScreen.svelte';
  import WelcomeScreen from './WelcomeScreen.svelte';
  import AuthShell from './AuthShell.svelte';

  const { Story } = defineMeta({
    title: 'Screens/Auth',
    tags: ['autodocs'],
    args: { preset: 'requires-login' },
    parameters: {
      docs: {
        description: {
          component:
            'Pre-login surfaces: LoginScreen (returning user), WelcomeScreen (first-user setup), and the AuthShell wrapper they share.',
        },
      },
    },
  });
</script>

<Story name="Login" args={{ preset: 'requires-login' }}>
  <LoginScreen />
</Story>

<Story name="Welcome (First User)" args={{ preset: 'requires-setup' }}>
  <WelcomeScreen />
</Story>

<Story name="Auth Shell" args={{ preset: 'empty' }}>
  <AuthShell title="Loading Koko" description="Checking server state and account access.">
    <p class="muted">Shell content goes here.</p>
  </AuthShell>
</Story>
