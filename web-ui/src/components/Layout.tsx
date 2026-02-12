import { Outlet, NavLink, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { api } from '../api';
import SyncStatus from './SyncStatus';

export default function Layout() {
  const navigate = useNavigate();
  const { data: status } = useQuery({
    queryKey: ['status'],
    queryFn: api.getStatus,
  });

  const handleLogout = async () => {
    try {
      await api.logout();
    } finally {
      localStorage.removeItem('session_token');
      navigate('/login');
    }
  };

  const navLinkClass = ({ isActive }: { isActive: boolean }) =>
    `px-3 py-2 rounded-md text-sm font-medium transition-colors ${
      isActive
        ? 'bg-gray-900 text-white'
        : 'text-gray-300 hover:bg-gray-700 hover:text-white'
    }`;

  return (
    <div className="min-h-screen bg-gray-100">
      <nav className="bg-gray-800">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between h-16">
            <div className="flex items-center">
              <span className="text-white font-bold text-lg">GitSvnSync</span>
              <div className="ml-10 flex items-baseline space-x-4">
                <NavLink to="/" end className={navLinkClass}>
                  Dashboard
                </NavLink>
                <NavLink to="/conflicts" className={navLinkClass}>
                  Conflicts
                  {status && status.active_conflicts > 0 && (
                    <span className="ml-2 inline-flex items-center justify-center px-2 py-1 text-xs font-bold leading-none text-red-100 bg-red-600 rounded-full">
                      {status.active_conflicts}
                    </span>
                  )}
                </NavLink>
                <NavLink to="/config" className={navLinkClass}>
                  Configuration
                </NavLink>
                <NavLink to="/audit" className={navLinkClass}>
                  Audit Log
                </NavLink>
              </div>
            </div>
            <div className="flex items-center space-x-4">
              {status && <SyncStatus status={status} />}
              <button
                onClick={handleLogout}
                className="text-gray-300 hover:text-white text-sm"
              >
                Logout
              </button>
            </div>
          </div>
        </div>
      </nav>

      <main className="max-w-7xl mx-auto py-6 sm:px-6 lg:px-8">
        <Outlet />
      </main>
    </div>
  );
}
