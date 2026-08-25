import { invoke } from '@tauri-apps/api/core'

export type ProfileMeta = {
  id: string
  name: string
  created_at_ms: number
  last_used_at_ms: number
}

export type ProfilesState = {
  active_profile_id: string
  profiles: ProfileMeta[]
}

export type ProfileSummary = {
  id: string
  name: string
  profile_image_url: string
  created_at_ms: number
  last_used_at_ms: number
  project_count: number
  terminal_count: number
  is_active: boolean
}

export async function listProfiles(): Promise<ProfilesState> {
  return invoke<ProfilesState>('list_profiles')
}

export async function listProfileSummaries(): Promise<ProfileSummary[]> {
  return invoke<ProfileSummary[]>('list_profile_summaries')
}

export async function getActiveProfile(): Promise<ProfileMeta> {
  return invoke<ProfileMeta>('get_active_profile')
}

export async function setActiveProfile(profileId: string): Promise<ProfilesState> {
  return invoke<ProfilesState>('set_active_profile', { profileId })
}

export async function createProfile(name?: string): Promise<ProfilesState> {
  return invoke<ProfilesState>('create_profile', { name })
}

export async function renameProfile(profileId: string, name: string): Promise<ProfilesState> {
  return invoke<ProfilesState>('rename_profile', { profileId, name })
}

export async function deleteProfile(profileId: string): Promise<ProfilesState> {
  return invoke<ProfilesState>('delete_profile', { profileId })
}
