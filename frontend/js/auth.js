// /pdf-bookstore/frontend/js/auth.js

// Authentication manager class untuk handle JWT authentication dan user session management
class AuthManager {
    constructor(apiClient = null) {
        this.api = apiClient || new BookstoreAPI();
        this.currentUser = null;
        this.authCallbacks = [];
        this.tokenCheckInterval = null;

        this.initializeAuthState();
        this.startTokenValidation();
    }

    // Function untuk initialize authentication state dengan stored token validation
    initializeAuthState() {
        const token = this.api.getStoredToken();
        const userData = localStorage.getItem('bookstore_user');

        if (token && userData) {
            try {
                this.currentUser = JSON.parse(userData);
                this.api.setToken(token);
            } catch (error) {
                console.error('Failed to parse stored user data:', error);
                this.clearAuth();
            }
        }
    }

    // Function untuk verify stored token dengan backend validation
    async verifyStoredToken() {
        try {
            // Verify token dengan auth service
            const response = await this.api.verifyToken();

            if (response.success && response.data) {
                let userData;
                if (response.data.user) {
                    userData = response.data.user;
                } else if (response.user) {
                    userData = response.user;
                } else {
                    throw new Error('Invalid token verification response structure');
                }

                this.currentUser = userData;
                this.triggerAuthCallbacks('login', this.currentUser);

                if (response.data && response.data.token_info) {
                    this.checkTokenExpiry(response.data.token_info);
                }

            } else {
                throw new Error('Token verification failed');
            }
        } catch (error) {
            // Token invalid, clear storage dan set guest state
            this.clearAuthData(); // hapus data token yang invalid
            throw new Error('Stored token is invalid');
        }
    }

    // Function untuk handle user login dengan credentials validation
    async login(email, password, remember = false) {
        try {
            console.log('AuthManager: Attempting login...');

            // Call API login method
            const response = await this.api.login(email, password);

            console.log('AuthManager: Login response:', response);

            // Handle successful login response
            if (response.success || response.token) {
                const token = response.token;
                const user = response.user;

                if (!token || !user) {
                    throw new Error('Invalid login response: missing token or user data');
                }

                // Store authentication data
                this.setAuthData(token, user, remember);

                console.log('AuthManager: Login successful, user:', user);

                // Notify auth callbacks
                this.notifyAuthCallbacks('login', user);

                return {
                    success: true,
                    user: user,
                    message: response.message || 'Login successful'
                };
            } else {
                throw new Error(response.message || 'Login failed');
            }
        } catch (error) {
            console.error('AuthManager: Login failed:', error);

            // Clear any partial auth state
            this.clearAuth();

            throw new Error(this.api.handleApiError(error));
        }
    }

    // Function untuk handle user registration dengan validation
    async register(userData) {
        try {
            console.log('AuthManager: Attempting registration...');

            const response = await this.api.register(userData);

            console.log('AuthManager: Registration response:', response);

            if (response.success) {
                // If registration includes token, set auth state
                if (response.token && response.user) {
                    this.setAuthData(response.token, response.user, false);
                    this.notifyAuthCallbacks('register', response.user);
                }

                return {
                    success: true,
                    user: response.user,
                    message: response.message || 'Registration successful'
                };
            } else {
                throw new Error(response.message || 'Registration failed');
            }
        } catch (error) {
            console.error('AuthManager: Registration failed:', error);
            throw new Error(this.api.handleApiError(error));
        }
    }

    // Function untuk handle user logout dengan session cleanup
    async logout() {
        try {
            console.log('AuthManager: Logging out...');

            const user = this.currentUser;
            const refreshToken = localStorage.getItem('bookstore_refresh_token');

            // Call backend logout untuk revoke tokens
            try {
                await this.api.logoutBackend(refreshToken);
                console.log('AuthManager: Backend logout successful');
            } catch (backendError) {
                console.warn('AuthManager: Backend logout failed, clearing local auth anyway:', backendError);
            }

            // Clear authentication state
            this.clearAuth();

            // Notify callbacks
            this.notifyAuthCallbacks('logout', user);

            console.log('AuthManager: Logout successful');

            return { success: true, message: 'Logged out successfully' };
        } catch (error) {
            console.error('AuthManager: Logout error:', error);
            // Still clear auth even if logout request fails
            this.clearAuth();
            return { success: true, message: 'Logged out (local only)' };
        }
    }

    clearAuth() {
        console.log('AuthManager: Clearing auth state');

        this.currentUser = null;
        this.api.clearAuth();

        localStorage.removeItem('bookstore_user');
        localStorage.removeItem('bookstore_remember');
        localStorage.removeItem('bookstore_token_expiry');
        localStorage.removeItem('bookstore_refresh_token');
        localStorage.removeItem('bookstore_session_token');
    }

    // Enhanced auth data management
    setAuthData(token, user, remember = false) {
        // Set token in API client
        this.api.setToken(token);

        // Store user data
        this.currentUser = user;
        localStorage.setItem('bookstore_user', JSON.stringify(user));

        // Handle remember me functionality
        if (remember) {
            localStorage.setItem('bookstore_remember', 'true');
            const extendedExpiry = Date.now() + (30 * 24 * 60 * 60 * 1000);
            localStorage.setItem('bookstore_token_expiry', extendedExpiry.toString());
        } else {
            localStorage.removeItem('bookstore_remember');
            localStorage.removeItem('bookstore_token_expiry');
        }

        console.log('AuthManager: Auth data set for user:', user.email);
    }

    isAuthenticated() {
        return this.api.isAuthenticated() && this.currentUser !== null;
    }

    // Function untuk cek current authentication status
    async checkAuthStatus() {
        // Cek apakah user sudah authenticated
        if (this.isAuthenticated()) {
            try {
                // Verify current token validity
                await this.verifyStoredToken();
                return {
                    isAuthenticated: true,
                    user: this.currentUser
                };
            } catch (error) {
                // Token invalid, logout user
                await this.logout();
                return {
                    isAuthenticated: false,
                    user: null
                };
            }
        } else {
            return {
                isAuthenticated: false,
                user: null
            };
        }
    }

    // Function untuk refresh JWT token untuk perpanjang session
    async refreshToken() {
        // Cek apakah user authenticated sebelum refresh
        if (!this.isAuthenticated()) {
            throw new Error('Tidak ada user yang authenticated untuk refresh token');
        }

        const refreshToken = localStorage.getItem('bookstore_refresh_token');
        if (!refreshToken) {
            throw new Error('No refresh token available');
        }

        try {
            // Call refresh token API
            const response = await this.api.refreshAccessToken(refreshToken);

            if (response.access_token) {
                // Update access token
                this.api.setToken(response.access_token);

                // Update refresh token if backend provides new one
                if (response.refresh_token) {
                    localStorage.setItem('bookstore_refresh_token', response.refresh_token);
                }

                console.log('AuthManager: Token refreshed successfully');

                return {
                    success: true,
                    message: 'Token berhasil di-refresh',
                    expires_in: response.expires_in
                };
            } else {
                throw new Error('Token refresh gagal - no access token in response');
            }
        } catch (error) {
            console.error('Token refresh failed:', error);
            // Force logout jika refresh gagal
            await this.logout();
            throw new Error('Session expired. Silakan login kembali.');
        }
    }

    // Function untuk setup automatic token refresh interval
    setupTokenRefresh() {
        // Clear existing refresh interval
        this.clearTokenRefresh();

        // Setup interval refresh baru
        this.tokenRefreshInterval = setInterval(async() => {
            try {
                await this.refreshToken();
            } catch (error) {
                console.warn('Auto token refresh gagal:', error);
            }
        }, this.config.tokenRefreshInterval);
    }

    // Function untuk clear automatic token refresh
    clearTokenRefresh() {
        if (this.tokenRefreshInterval) {
            clearInterval(this.tokenRefreshInterval);
            this.tokenRefreshInterval = null;
        }
    }

    // Function untuk setup session timeout warning
    setupSessionTimeout() {

        this.clearSessionTimeout();

        this.sessionTimeoutWarning = setTimeout(() => {
            // Show session timeout warning
            if (this.isAuthenticated()) {
                this.showSessionTimeoutWarning();
            }
        }, this.config.sessionWarningTime);
    }

    // Function untuk clear session timeout warning
    clearSessionTimeout() {
        if (this.sessionTimeoutWarning) {
            clearTimeout(this.sessionTimeoutWarning);
            this.sessionTimeoutWarning = null;
        }
    }

    // Function untuk cek apakah user sedang authenticated
    isAuthenticated() {
        return this.api.isAuthenticated() && this.currentUser !== null;
    }

    // Function untuk get current authenticated user
    getCurrentUser() {
        if (this.currentUser) {
            return this.currentUser;
        }

        const userData = localStorage.getItem('bookstore_user');
        if (userData) {
            try {
                this.currentUser = JSON.parse(userData);
                return this.currentUser;
            } catch (error) {
                console.error('Failed to parse stored user data:', error);
                this.clearAuth();
            }
        }

        return null;
    }

    isAdmin() {
        const user = this.getCurrentUser();
        return user && user.role === 'admin';
    }

    async validateToken() {
        if (!this.api.isAuthenticated()) {
            return false;
        }

        try {
            const isValid = await this.api.verifyToken();

            if (!isValid) {
                console.log('AuthManager: Token validation failed, clearing auth');
                this.clearAuth();
                this.notifyAuthCallbacks('tokenExpired', null);
                return false;
            }

            return true;
        } catch (error) {
            console.error('AuthManager: Token validation error:', error);
            this.clearAuth();
            this.notifyAuthCallbacks('tokenExpired', null);
            return false;
        }
    }

    startTokenValidation() {
        if (this.tokenCheckInterval) {
            clearInterval(this.tokenCheckInterval);
        }

        this.tokenCheckInterval = setInterval(async() => {
            if (this.isAuthenticated()) {
                const isValid = await this.validateToken();
                if (!isValid) {
                    console.log('AuthManager: Token expired during periodic check');
                    // Redirect to login if on a protected page
                    if (this.isProtectedPage()) {
                        this.redirectToLogin();
                    }
                }
            }
        }, 5 * 60 * 1000);
    }

    stopTokenValidation() {
        if (this.tokenCheckInterval) {
            clearInterval(this.tokenCheckInterval);
            this.tokenCheckInterval = null;
        }
    }

    async fetchUserProfile() {
        if (!this.isAuthenticated()) {
            throw new Error('Not authenticated');
        }

        try {
            const response = await this.api.getProfile();

            if (response.success && response.user) {
                this.currentUser = response.user;
                localStorage.setItem('bookstore_user', JSON.stringify(response.user));

                this.notifyAuthCallbacks('profileUpdated', response.user);
                return response.user;
            } else {
                throw new Error(response.message || 'Failed to fetch profile');
            }
        } catch (error) {
            console.error('AuthManager: Failed to fetch profile:', error);
            if (error.message.includes('token') || error.message.includes('401')) {
                this.clearAuth();
                this.notifyAuthCallbacks('tokenExpired', null);
            }

            throw new Error(this.api.handleApiError(error));
        }
    }

    // ===== CALLBACK MANAGEMENT =====

    // Add authentication event callback
    addAuthCallback(callback) {
        if (typeof callback === 'function') {
            this.authCallbacks.push(callback);
        }
    }

    // Remove authentication event callback
    removeAuthCallback(callback) {
        this.authCallbacks = this.authCallbacks.filter(cb => cb !== callback);
    }

    // Notify all auth callbacks
    notifyAuthCallbacks(event, user) {
        this.authCallbacks.forEach(callback => {
            try {
                callback(event, user);
            } catch (error) {
                console.error('AuthManager: Callback error:', error);
            }
        });
    }

    // ===== UTILITY METHODS =====
    // Check if current page requires authentication
    isProtectedPage() {
        const protectedPaths = ['/admin/', '/profile/', '/library/', '/checkout.'];
        const currentPath = window.location.pathname.toLowerCase();

        return protectedPaths.some(path => currentPath.includes(path));
    }

    // Redirect to login with return URL
    redirectToLogin(returnUrl = null) {
        const url = returnUrl || window.location.pathname + window.location.search;
        const loginUrl = `/login.html?redirect=${encodeURIComponent(url)}`;
        window.location.href = loginUrl;
    }

    // Require authentication for protected actions
    requireAuth(action = 'perform this action') {
        if (!this.isAuthenticated()) {
            throw new Error(`Please log in to ${action}`);
        }
        return true;
    }

    // Require admin role for admin actions
    requireAdmin(action = 'perform this action') {
        this.requireAuth(action);

        if (!this.isAdmin()) {
            throw new Error(`Admin privileges required to ${action}`);
        }
        return true;
    }

    // Get user display name
    getUserDisplayName() {
        const user = this.getCurrentUser();
        if (!user) return 'Guest';

        return user.full_name || user.name || user.email.split('@')[0];
    }

    // Get user avatar URL or initials
    getUserAvatar() {
        const user = this.getCurrentUser();
        if (!user) return null;

        // Return initials if no avatar URL
        if (user.avatar_url) {
            return user.avatar_url;
        }

        const name = user.full_name || user.email;
        const initials = name.split(' ')
            .map(word => word.charAt(0).toUpperCase())
            .slice(0, 2)
            .join('');

        return initials;
    }

    // Check token expiry
    isTokenExpired() {
        const timestamp = localStorage.getItem('bookstore_token_timestamp');
        const expiry = localStorage.getItem('bookstore_token_expiry');

        if (expiry) {
            return Date.now() > parseInt(expiry);
        }

        if (timestamp) {
            const tokenAge = Date.now() - parseInt(timestamp);
            const maxAge = 24 * 60 * 60 * 1000; // 24 hours default
            return tokenAge > maxAge;
        }

        return true;
    }

    // Function untuk handle guest state setup
    handleGuestState() {
        this.currentUser = null;
        this.triggerAuthCallbacks('guest', null);
    }

    // Function untuk handle authentication errors
    handleAuthError(error) {
        console.error('Authentication error:', error);
        this.clearAuthData();
        this.triggerAuthCallbacks('error', error);
    }

    // Function untuk validasi format email dengan comprehensive regex
    validateEmail(email) {
        const emailRegex = /^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/;


        return emailRegex.test(email);
    }

    // Function untuk validasi kekuatan password dengan multiple criteria
    validatePassword(password) {
        const result = {
            isValid: false,
            errors: [],
            strength: 'lemah'
        };

        // Validasi panjang password
        if (password.length < 6) {
            result.errors.push('Password minimal 6 karakter');
        }
        if (password.length > 128) {
            result.errors.push('Password maksimal 128 karakter');
        }

        // Perhitungan kekuatan password
        let strengthScore = 0;
        if (password.length >= 8) strengthScore++;
        if (/[a-z]/.test(password)) strengthScore++;
        if (/[A-Z]/.test(password)) strengthScore++;
        if (/[0-9]/.test(password)) strengthScore++;
        if (/[^a-zA-Z0-9]/.test(password)) strengthScore++;

        // Set level kekuatan berdasarkan score
        if (strengthScore >= 4) {
            result.strength = 'kuat';
        } else if (strengthScore >= 2) {
            result.strength = 'sedang';
        }

        // Cek pattern lemah yang umum
        const weakPatterns = [/^123456/, /password/i, /^qwerty/i];
        if (weakPatterns.some(pattern => pattern.test(password))) {
            result.errors.push('Password mengandung pattern yang lemah');
        }

        // Validasi final
        result.isValid = result.errors.length === 0 && strengthScore >= 2;
        return result;
    }

    // Function untuk handle login rate limiting
    isLoginLocked() {
        const attempts = this.getFailedLoginAttempts();
        const lastAttempt = localStorage.getItem('last_failed_login');

        if (attempts >= this.config.maxLoginAttempts && lastAttempt) {
            const timeSinceLastAttempt = Date.now() - parseInt(lastAttempt);
            return timeSinceLastAttempt < this.config.lockoutDuration;
        }
        return false;
    }

    // Function untuk get sisa waktu lockout
    getLockTimeRemaining() {
        const lastAttempt = localStorage.getItem('last_failed_login');
        if (lastAttempt) {
            const elapsed = Date.now() - parseInt(lastAttempt);
            const remaining = this.config.lockoutDuration - elapsed;
            return Math.ceil(remaining / (60 * 1000));
        }
        return 0;
    }

    // Function untuk get jumlah percobaan login gagal
    getFailedLoginAttempts() {
        return parseInt(localStorage.getItem('failed_login_attempts') || '0');
    }

    // Function untuk record failed login attempt
    recordFailedLogin() {
        const attempts = this.getFailedLoginAttempts() + 1;
        localStorage.setItem('failed_login_attempts', attempts.toString());
        localStorage.setItem('last_failed_login', Date.now().toString());
    }

    // Function untuk clear failed login attempts
    clearLoginAttempts() {
        localStorage.removeItem('failed_login_attempts');
        localStorage.removeItem('last_failed_login');
    }

    // Function untuk enable remember me functionality
    enableRememberMe() {

        const extendedExpiry = Date.now() + (30 * 24 * 60 * 60 * 1000);
        localStorage.setItem('remember_me_expiry', extendedExpiry.toString());
    }

    // Function untuk cek token expiry
    checkTokenExpiry(tokenInfo) {
        if (tokenInfo && tokenInfo.exp) {
            const expiryTime = tokenInfo.exp * 1000;
            const currentTime = Date.now();
            const timeUntilExpiry = expiryTime - currentTime;

            // Jika kurang dari 10 menit, refresh sekarang
            if (timeUntilExpiry < 10 * 60 * 1000) {
                this.refreshToken().catch(console.error);
            }
        }
    }

    // Function untuk show session timeout warning
    showSessionTimeoutWarning() {
        // Bisa implement modal warning di sini
        console.warn('Session akan expired dalam 5 menit');
        // TODO: Implement modal warning untuk user
    }

    // Function untuk register callback untuk login events
    onLogin(callback) {
        this.loginCallbacks.push(callback);
    }

    // Function untuk register callback untuk logout events
    onLogout(callback) {
        this.logoutCallbacks.push(callback);
    }

    // Function untuk register callback untuk auth events
    onAuthChange(callback) {
        this.authCallbacks.push(callback);
    }

    // Function untuk remove callback dari auth events
    removeAuthCallback(callback) {
        this.authCallbacks = this.authCallbacks.filter(cb => cb !== callback);
        this.loginCallbacks = this.loginCallbacks.filter(cb => cb !== callback);
        this.logoutCallbacks = this.logoutCallbacks.filter(cb => cb !== callback);
    }

    // Function untuk trigger semua registered auth callbacks
    triggerAuthCallbacks(eventType, userData) {
        this.authCallbacks.forEach(callback => {
            try {
                // Call callback dengan event type dan user data
                callback(eventType, userData);
            } catch (error) {
                console.error('Auth callback error:', error);
            }
        });
    }

    // Function untuk trigger login callbacks
    triggerLoginCallbacks(userData) {
        this.loginCallbacks.forEach(callback => {
            try {
                callback(userData);
            } catch (error) {
                console.error('Login callback error:', error);
            }
        });
    }

    // Function untuk trigger logout callbacks
    triggerLogoutCallbacks() {
        this.logoutCallbacks.forEach(callback => {
            try {
                callback();
            } catch (error) {
                console.error('Logout callback error:', error);
            }
        });
    }


    // ===== INITIALIZATION METHODS =====

    // Initialize auth on page load
    async initializeOnPageLoad() {
        try {
            // Check if user is already authenticated
            if (this.isAuthenticated()) {
                // Validate token on page load
                const isValid = await this.validateToken();

                if (isValid) {
                    console.log('AuthManager: Already authenticated:', this.currentUser.email);
                    this.notifyAuthCallbacks('ready', this.currentUser);
                    return true;
                } else {
                    console.log('AuthManager: Stored token is invalid');
                    return false;
                }
            } else {
                console.log('AuthManager: No authentication found');
                this.notifyAuthCallbacks('ready', null);
                return false;
            }
        } catch (error) {
            console.error('AuthManager: Initialization error:', error);
            this.clearAuth();
            this.notifyAuthCallbacks('ready', null);
            return false;
        }
    }


    // Function untuk cleanup auth resources
    cleanup() {
        this.stopTokenValidation();
        this.authCallbacks = [];
    }


}

// Export AuthManager class untuk global access
window.AuthManager = AuthManager;
window.authManager = new AuthManager();

document.addEventListener('DOMContentLoaded', () => {
    window.authManager.initializeOnPageLoad();
});

window.addEventListener('beforeunload', () => {
    window.authManager.cleanup();
});