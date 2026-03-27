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
        ? 'bg-blue-600 text-white'
        : 'text-gray-400 hover:bg-gray-700 hover:text-white'
    }`;

  return (
    <div className="min-h-screen bg-gray-900">
      <nav className="bg-gray-950 border-b border-gray-800">
        <div className="px-6 sm:px-8 lg:px-12">
          <div className="flex items-center justify-between h-16">
            <div className="flex items-center">
              <svg className="w-8 h-8" viewBox="0 0 56 56" fill="none" xmlns="http://www.w3.org/2000/svg">
                <defs>
                  <linearGradient id="navLogoGrad" x1="0" y1="0" x2="56" y2="56" gradientUnits="userSpaceOnUse">
                    <stop offset="0%" stopColor="#3b82f6"/>
                    <stop offset="100%" stopColor="#818cf8"/>
                  </linearGradient>
                  <linearGradient id="navArrowGrad" x1="10" y1="10" x2="46" y2="46" gradientUnits="userSpaceOnUse">
                    <stop offset="0%" stopColor="#60a5fa"/>
                    <stop offset="100%" stopColor="#a78bfa"/>
                  </linearGradient>
                </defs>
                <circle cx="28" cy="28" r="25" stroke="url(#navLogoGrad)" strokeWidth="2" fill="none" opacity="0.3"/>
                <path d="M 38 14 A 16 16 0 0 1 42 28" stroke="url(#navArrowGrad)" strokeWidth="2.5" strokeLinecap="round" fill="none"/>
                <polygon points="43,27 42,31 39,28" fill="#60a5fa"/>
                <path d="M 18 42 A 16 16 0 0 1 14 28" stroke="url(#navArrowGrad)" strokeWidth="2.5" strokeLinecap="round" fill="none"/>
                <polygon points="13,29 14,25 17,28" fill="#a78bfa"/>
                <circle cx="28" cy="28" r="5" fill="#1e293b" stroke="url(#navLogoGrad)" strokeWidth="2"/>
                <circle cx="28" cy="28" r="2" fill="#60a5fa"/>
                <line x1="28" y1="23" x2="28" y2="14" stroke="#3b82f6" strokeWidth="1.5" strokeLinecap="round" opacity="0.7"/>
                <line x1="28" y1="33" x2="28" y2="42" stroke="#818cf8" strokeWidth="1.5" strokeLinecap="round" opacity="0.7"/>
                <circle cx="28" cy="13" r="2" fill="#3b82f6"/>
                <circle cx="28" cy="43" r="2" fill="#818cf8"/>
              </svg>
              <span className="text-white font-bold text-lg font-display tracking-wider">RepoSync</span>
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
                <NavLink to="/setup" className={navLinkClass}>
                  Setup
                </NavLink>
              </div>
            </div>
            <div className="flex items-center space-x-4">
              {status && <SyncStatus status={status} />}
              <button
                onClick={handleLogout}
                className="text-gray-400 hover:text-white text-sm"
              >
                Logout
              </button>
            </div>
          </div>
        </div>
      </nav>

      <main className="py-6 px-6 sm:px-8 lg:px-12">
        <Outlet />
      </main>
    </div>
  );
}
