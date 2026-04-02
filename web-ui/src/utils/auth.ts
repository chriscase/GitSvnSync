export interface StoredUser {
  id: string;
  username: string;
  role: string;
  display_name?: string;
  email?: string;
}

export function getStoredUser(): StoredUser | null {
  try {
    const stored = localStorage.getItem('user');
    return stored ? JSON.parse(stored) : null;
  } catch {
    return null;
  }
}
