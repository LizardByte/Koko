<script module lang="ts">
  // Settings stories — the settings UI is store-driven (settings/activities/auth
  // stores, seeded by the 'settings' preset). Each story renders one section.
  // The mock API provides settings fixtures (3 libraries, 3 providers, scheduled
  // tasks, logs, dashboard items).
  import { defineMeta } from '@storybook/addon-svelte-csf';
  import SettingsShell from './SettingsShell.svelte';
  import GeneralForm from './GeneralForm.svelte';
  import UserManagement from './UserManagement.svelte';
  import LibrarySettings from './LibrarySettings.svelte';
  import ProviderSettings from './ProviderSettings.svelte';
  import ScheduledTasks from './ScheduledTasks.svelte';
  import MetadataDashboard from './MetadataDashboard.svelte';
  import SystemActivities from './SystemActivities.svelte';
  import LogViewer from './LogViewer.svelte';

  const { Story } = defineMeta({
    title: 'Screens/Settings',
    tags: ['autodocs'],
    args: { preset: 'settings' },
    parameters: {
      docs: {
        description: {
          component:
            'Settings hub: 6 sub-pages (general+users, libraries, providers, scheduled tasks, dashboard, logs). Store-driven — all data comes from the settings/activities/auth stores, seeded by the `settings` preset. The mock API provides realistic fixtures.',
        },
      },
    },
  });
</script>

<!--
  Each story wraps the section in a SettingsShell so the navbar + section nav
  renders. The section nav highlights based on the `route` arg.
-->

<Story name="General + Users" args={{ preset: 'settings', route: '/settings' }} asChild>
  <SettingsShell>
    <GeneralForm />
    <UserManagement />
  </SettingsShell>
</Story>

<Story name="Libraries" args={{ preset: 'settings', route: '/settings/libraries' }} asChild>
  <SettingsShell>
    <LibrarySettings />
  </SettingsShell>
</Story>

<Story name="Providers" args={{ preset: 'settings', route: '/settings/providers' }} asChild>
  <SettingsShell>
    <ProviderSettings />
  </SettingsShell>
</Story>

<Story name="Scheduled Tasks" args={{ preset: 'settings', route: '/settings/scheduled' }} asChild>
  <SettingsShell>
    <ScheduledTasks />
  </SettingsShell>
</Story>

<Story name="Dashboard" args={{ preset: 'settings', route: '/settings/dashboard' }} asChild>
  <SettingsShell>
    <MetadataDashboard />
    <SystemActivities />
  </SettingsShell>
</Story>

<Story name="Logs" args={{ preset: 'settings', route: '/settings/logs' }} asChild>
  <SettingsShell>
    <LogViewer />
  </SettingsShell>
</Story>
