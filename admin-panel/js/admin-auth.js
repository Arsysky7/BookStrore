// /pdf-bookstore/admin-panel/js/admin-auth.js

// Admin Authentication class untuk handle admin login dengan role validation
class AdminAuthentication {
    constructor() {
        this.api = new BookstoreAPI(); // API client dari frontend/js/api.js line 3
        this.auth = new AuthManager(this.api); // Auth manager dari frontend/js/auth.js line 3
        this.currentAdmin = null;

        // Configuration untuk admin authentication
        this.config = {
            requiredRole: 'admin', // role yang diperlukan untuk access admin panel
            redirectAfterLogin: 'dashboard.html', // halaman tujuan setelah login berhasil
            sessionTimeout: 60 * 60 * 1000, // 1 hour session timeout untuk admin
            maxLoginAttempts: 3 // max percobaan login untuk admin
        };

        // Initialize admin authentication flow
        this.initializeAdminAuth();
    }

    // Function untuk initialize admin authentication dengan role checking
    async initializeAdminAuth() {
        try {
            // Setup event listeners untuk login form
            this.setupEventListeners();

            // Setup demo credentials functionality
            this.setupDemoCredentials();

            // Check existing authentication status
            await this.checkExistingAuth();

            // Setup password visibility toggle
            this.setupPasswordToggle();

        } catch (error) {
            console.error('Admin auth initialization failed:', error);
            Utils.showNotification('Failed to initialize admin login', 'error');
        }
    }

    // Function untuk setup event listeners untuk admin login form
    setupEventListeners() {
        // Admin login form submission
        const loginForm = document.getElementById('admin-login-form');
        // ^^^^^^^^^ = form element dari index.html line 45

        if (loginForm) {
            loginForm.addEventListener('submit', this.handleAdminLogin.bind(this));
            // ^^^^^^^^^^^^^^^^^^^^^^^^^ = method untuk handle login process
        }

        // Real-time form validation
        this.setupFormValidation();

        // Forgot password link handler
        const forgotPasswordLink = document.getElementById('forgot-password-link');
        if (forgotPasswordLink) {
            forgotPasswordLink.addEventListener('click', this.handleForgotPassword.bind(this));
        }
    }

    // Function untuk handle admin login dengan role validation
    async handleAdminLogin(event) {
        event.preventDefault();
        const formData = new FormData(event.target);

        // Extract form data
        const email = formData.get('email');
        const password = formData.get('password');
        const rememberMe = formData.get('remember_me') === 'on';

        // Clear previous errors
        this.clearFormErrors();

        // Validate input fields
        if (!this.validateAdminLoginInput(email, password)) {
            return; // validation errors akan ditampilkan
        }

        try {
            Utils.showLoading('Authenticating admin credentials...');

            // Attempt login menggunakan auth service dari auth-service/handlers.rs line 45
            const loginResult = await this.auth.login(email, password, rememberMe);
            // ^^^^^^^^^^^ = result dari AuthResponse struct

            if (loginResult.success && loginResult.user) {
                // Validate admin role dari user data
                await this.validateAdminRole(loginResult.user);
                // ^^^^^^^^^^^^^^^^^^^ = check role 'admin' dari User struct

                // Store admin user data
                this.currentAdmin = loginResult.user;

                // Setup admin session management
                this.setupAdminSession();

                // Show success notification
                Utils.showNotification('Admin login successful! Redirecting...', 'success');

                // Redirect ke admin dashboard setelah delay singkat
                setTimeout(() => {
                    window.location.href = this.config.redirectAfterLogin;
                }, 1500);

            } else {
                throw new Error(loginResult.message || 'Login failed');
            }

        } catch (error) {
            console.error('Admin login failed:', error);

            // Handle specific admin login errors
            this.handleAdminLoginError(error);

        } finally {
            Utils.hideLoading();
        }
    }

    // Function untuk validate admin role dari authenticated user
    async validateAdminRole(user) {
        // Check role field dari User struct di auth-service/models.rs line 8
        if (!user.role || user.role !== this.config.requiredRole) {
            // Clear authentication jika bukan admin
            await this.auth.logout();

            throw new Error('Access denied. Admin privileges required.');
        }

        // Additional admin validation bisa ditambahkan di sini
        if (!user.is_active) {
            await this.auth.logout();
            throw new Error('Admin account is disabled. Contact system administrator.');
        }

        // Log admin access untuk security audit
        this.logAdminAccess(user);
    }

    // Function untuk validate input fields dengan admin-specific rules
    validateAdminLoginInput(email, password) {
        let isValid = true;

        // Email validation dengan enhanced checking untuk admin
        if (!email) {
            this.showFieldError('email', 'Email address is required');
            isValid = false;
        } else if (!Utils.isValidEmail(email)) {
            this.showFieldError('email', 'Please enter a valid email address');
            isValid = false;
        } else if (!email.includes('@')) { // Additional admin email validation
            this.showFieldError('email', 'Admin email must be a valid corporate email');
            isValid = false;
        }

        // Password validation dengan admin security requirements
        if (!password) {
            this.showFieldError('password', 'Password is required');
            isValid = false;
        } else if (password.length < 6) {
            this.showFieldError('password', 'Password must be at least 6 characters');
            isValid = false;
        }

        // Check login attempts untuk prevent brute force
        if (this.isAdminLoginLocked()) {
            const remainingTime = this.getAdminLockTimeRemaining();
            Utils.showNotification(
                `Too many failed attempts. Try again in ${remainingTime} minutes.`,
                'warning'
            );
            isValid = false;
        }

        return isValid;
    }

    // Function untuk handle admin login errors dengan specific messaging
    handleAdminLoginError(error) {
        // Increment failed login attempts untuk security
        this.incrementFailedAttempts();

        // Map error messages ke user-friendly admin messages
        const adminErrorMessages = {
            'Invalid email or password': 'Invalid admin credentials. Please check your email and password.',
            'Access denied': 'Admin access denied. Contact system administrator if you believe this is an error.',
            'Account disabled': 'Your admin account has been disabled. Contact system administrator.',
            'Session expired': 'Your session has expired. Please login again.',
            'Network error': 'Unable to connect to authentication server. Please try again.',
        };

        // Get appropriate error message
        const userMessage = adminErrorMessages[error.message] ||
            'Admin login failed. Please verify your credentials and try again.';

        // Show error notification
        Utils.showNotification(userMessage, 'error');

        // Clear sensitive form data pada error
        this.clearSensitiveData();
    }

    // Function untuk setup admin session management dengan enhanced security
    setupAdminSession() {
        // Set admin-specific session timeout
        this.adminSessionTimeout = setTimeout(() => {
            this.handleAdminSessionExpiry();
        }, this.config.sessionTimeout);

        // Setup periodic session validation untuk admin
        this.adminSessionCheck = setInterval(async() => {
            try {
                await this.validateAdminSession();
            } catch (error) {
                console.warn('Admin session validation failed:', error);
                this.handleAdminSessionExpiry();
            }
        }, 5 * 60 * 1000); // Check every 5 minutes

        // Store admin session info
        sessionStorage.setItem('admin_session_start', Date.now().toString());
        sessionStorage.setItem('admin_user_role', this.currentAdmin.role);
    }

    // Function untuk validate ongoing admin session
    async validateAdminSession() {
        if (!this.auth.isAuthenticated()) {
            throw new Error('Admin session not authenticated');
        }

        // Verify admin role masih valid
        const currentUser = this.auth.getCurrentUser();
        if (!currentUser || currentUser.role !== this.config.requiredRole) {
            throw new Error('Admin role validation failed');
        }

        // Additional admin-specific session checks bisa ditambahkan
        return true;
    }

    // Function untuk handle admin session expiry
    async handleAdminSessionExpiry() {
        // Clear session timers
        if (this.adminSessionTimeout) {
            clearTimeout(this.adminSessionTimeout);
        }
        if (this.adminSessionCheck) {
            clearInterval(this.adminSessionCheck);
        }

        // Logout admin user
        await this.auth.logout();

        // Clear admin data
        this.currentAdmin = null;
        sessionStorage.clear();

        // Show expiry notification
        Utils.showNotification(
            'Admin session expired for security. Please login again.',
            'warning'
        );

        // Redirect ke login page
        setTimeout(() => {
            window.location.href = 'index.html';
        }, 2000);
    }

    // Function untuk check existing authentication pada page load
    async checkExistingAuth() {
        try {
            // Check stored authentication
            const authStatus = await this.auth.checkAuthStatus();

            if (authStatus.isAuthenticated && authStatus.user) {
                // Validate admin role untuk existing session
                if (authStatus.user.role === this.config.requiredRole) {
                    this.currentAdmin = authStatus.user;

                    // Redirect ke dashboard jika sudah authenticated sebagai admin
                    Utils.showNotification('Welcome back, Admin!', 'info');
                    setTimeout(() => {
                        window.location.href = this.config.redirectAfterLogin;
                    }, 1000);
                } else {
                    // Logout jika bukan admin role
                    await this.auth.logout();
                }
            }
        } catch (error) {
            console.warn('Existing auth check failed:', error);
            // Clear any invalid stored auth
            await this.auth.logout();
        }
    }

    // Function untuk setup demo credentials functionality
    setupDemoCredentials() {
        // Copy button functionality untuk demo credentials
        const copyButtons = document.querySelectorAll('.copy-btn');
        // ^^^^^^^^^^^ = buttons dari index.html line 132

        copyButtons.forEach(button => {
            button.addEventListener('click', async(e) => {
                const textToCopy = e.target.dataset.copy || e.target.closest('.copy-btn').dataset.copy;
                // ^^^^^^^^^ = data dari copy button attribute

                try {
                    const success = await Utils.copyToClipboard(textToCopy);
                    // ^^^^^^^ = utils function dari utils.js line 234

                    if (success) {
                        // Update button icon untuk feedback
                        const icon = button.querySelector('i');
                        const originalClass = icon.className;

                        icon.className = 'fas fa-check';
                        button.style.color = 'var(--admin-secondary)';

                        // Reset setelah 2 detik
                        setTimeout(() => {
                            icon.className = originalClass;
                            button.style.color = '';
                        }, 2000);

                        Utils.showNotification('Copied to clipboard!', 'success');
                    }
                } catch (error) {
                    Utils.showNotification('Failed to copy to clipboard', 'error');
                }
            });
        });

        // Auto-fill demo credentials on click
        const demoCredentials = document.querySelector('.demo-credentials');
        if (demoCredentials) {
            demoCredentials.addEventListener('click', this.handleDemoCredentialClick.bind(this));
        }
    }

    // Function untuk handle demo credential click untuk auto-fill
    handleDemoCredentialClick(e) {
        if (e.target.closest('.credential-item')) {
            const credentialItem = e.target.closest('.credential-item');
            const credentialValue = credentialItem.querySelector('.credential-value').textContent;
            const credentialLabel = credentialItem.querySelector('.credential-label').textContent.toLowerCase();

            // Auto-fill form fields berdasarkan credential type
            if (credentialLabel.includes('email')) {
                const emailInput = document.getElementById('email');
                if (emailInput) {
                    emailInput.value = credentialValue;
                    emailInput.focus();
                }
            } else if (credentialLabel.includes('password')) {
                const passwordInput = document.getElementById('password');
                if (passwordInput) {
                    passwordInput.value = credentialValue;
                    passwordInput.focus();
                }
            }

            // Clear any existing validation errors
            this.clearFormErrors();
        }
    }

    // Function untuk setup password visibility toggle
    setupPasswordToggle() {
        const passwordToggle = document.getElementById('password-toggle');
        const passwordInput = document.getElementById('password');

        if (passwordToggle && passwordInput) {
            passwordToggle.addEventListener('click', () => {
                const isPassword = passwordInput.type === 'password';

                // Toggle input type
                passwordInput.type = isPassword ? 'text' : 'password';

                // Toggle icon
                const icon = passwordToggle.querySelector('i');
                icon.className = isPassword ? 'fas fa-eye-slash' : 'fas fa-eye';
            });
        }
    }

    // Function untuk setup real-time form validation
    setupFormValidation() {
        const emailInput = document.getElementById('email');
        const passwordInput = document.getElementById('password');

        // Email validation pada blur
        if (emailInput) {
            emailInput.addEventListener('blur', () => {
                const email = emailInput.value.trim();
                if (email && !Utils.isValidEmail(email)) {
                    this.showFieldError('email', 'Please enter a valid email address');
                } else {
                    this.clearFieldError('email');
                }
            });

            // Clear error saat user mulai typing
            emailInput.addEventListener('input', () => {
                this.clearFieldError('email');
            });
        }

        // Password validation pada blur
        if (passwordInput) {
            passwordInput.addEventListener('blur', () => {
                const password = passwordInput.value;
                if (password && password.length < 6) {
                    this.showFieldError('password', 'Password must be at least 6 characters');
                } else {
                    this.clearFieldError('password');
                }
            });

            // Clear error saat user mulai typing
            passwordInput.addEventListener('input', () => {
                this.clearFieldError('password');
            });
        }
    }

    // Function untuk show field-specific error messages
    showFieldError(fieldName, message) {
        const errorElement = document.getElementById(`${fieldName}-error`);
        const inputElement = document.getElementById(fieldName);

        if (errorElement) {
            errorElement.textContent = message;
            errorElement.style.display = 'block';
        }

        if (inputElement) {
            inputElement.style.borderColor = 'var(--admin-danger)';
            inputElement.style.boxShadow = '0 0 0 3px rgba(239, 68, 68, 0.1)';
        }
    }

    // Function untuk clear field-specific error messages
    clearFieldError(fieldName) {
        const errorElement = document.getElementById(`${fieldName}-error`);
        const inputElement = document.getElementById(fieldName);

        if (errorElement) {
            errorElement.textContent = '';
            errorElement.style.display = 'none';
        }

        if (inputElement) {
            inputElement.style.borderColor = '';
            inputElement.style.boxShadow = '';
        }
    }

    // Function untuk clear semua form errors
    clearFormErrors() {
        const errorElements = document.querySelectorAll('.form-error');
        const inputElements = document.querySelectorAll('.form-input');

        errorElements.forEach(error => {
            error.textContent = '';
            error.style.display = 'none';
        });

        inputElements.forEach(input => {
            input.style.borderColor = '';
            input.style.boxShadow = '';
        });
    }

    // Function untuk clear sensitive form data
    clearSensitiveData() {
        const passwordInput = document.getElementById('password');
        if (passwordInput) {
            passwordInput.value = '';
        }
    }

    // Function untuk handle forgot password functionality
    handleForgotPassword(e) {
        e.preventDefault();

        // Show information modal about admin password recovery
        Utils.showNotification(
            'For admin password recovery, please contact the system administrator at support@bookstore.com',
            'info',
            8000 // longer duration untuk important info
        );
    }

    // Function untuk check admin login rate limiting
    isAdminLoginLocked() {
        const attempts = this.getFailedAdminAttempts();
        const lastAttempt = localStorage.getItem('admin_last_failed_login');

        if (attempts >= this.config.maxLoginAttempts && lastAttempt) {
            const timeSinceLastAttempt = Date.now() - parseInt(lastAttempt);
            const lockoutDuration = 15 * 60 * 1000; // 15 minutes lockout untuk admin
            return timeSinceLastAttempt < lockoutDuration;
        }

        return false;
    }

    // Function untuk get remaining lockout time untuk admin
    getAdminLockTimeRemaining() {
        const lastAttempt = localStorage.getItem('admin_last_failed_login');
        if (lastAttempt) {
            const elapsed = Date.now() - parseInt(lastAttempt);
            const lockoutDuration = 15 * 60 * 1000; // 15 minutes
            const remaining = lockoutDuration - elapsed;
            return Math.ceil(remaining / (60 * 1000)); // return minutes
        }
        return 0;
    }

    // Function untuk get failed admin login attempts
    getFailedAdminAttempts() {
        return parseInt(localStorage.getItem('admin_failed_attempts') || '0');
    }

    // Function untuk increment failed admin login attempts
    incrementFailedAttempts() {
        const attempts = this.getFailedAdminAttempts() + 1;
        localStorage.setItem('admin_failed_attempts', attempts.toString());
        localStorage.setItem('admin_last_failed_login', Date.now().toString());

        // Show warning jika mendekati lockout
        if (attempts >= this.config.maxLoginAttempts - 1) {
            Utils.showNotification(
                `Warning: Account will be locked after ${this.config.maxLoginAttempts - attempts} more failed attempts`,
                'warning'
            );
        }
    }

    // Function untuk clear failed admin login attempts
    clearFailedAttempts() {
        localStorage.removeItem('admin_failed_attempts');
        localStorage.removeItem('admin_last_failed_login');
    }

    // Function untuk log admin access untuk security audit
    logAdminAccess(adminUser) {
        const accessLog = {
            admin_id: adminUser.id,
            admin_email: adminUser.email,
            access_time: new Date().toISOString(),
            ip_address: 'unknown', // Could be enhanced dengan IP detection
            user_agent: navigator.userAgent,
            session_id: this.generateSessionId()
        };

        // Store locally untuk demo (production bisa kirim ke logging service)
        const existingLogs = JSON.parse(localStorage.getItem('admin_access_logs') || '[]');
        existingLogs.push(accessLog);

        // Keep only last 50 logs untuk prevent storage bloat
        if (existingLogs.length > 50) {
            existingLogs.splice(0, existingLogs.length - 50);
        }

        localStorage.setItem('admin_access_logs', JSON.stringify(existingLogs));

        console.log('Admin access logged:', accessLog);
    }

    // Function untuk generate unique session ID
    generateSessionId() {
        return 'admin_' + Date.now().toString(36) + '_' + Math.random().toString(36).substr(2, 9);
    }

    // Function untuk get current admin user
    getCurrentAdmin() {
        return this.currentAdmin;
    }

    // Function untuk check jika user adalah authenticated admin
    isAdminAuthenticated() {
        return !!(this.currentAdmin && this.currentAdmin.role === this.config.requiredRole);
    }

    // Function untuk cleanup admin authentication resources
    cleanup() {
        // Clear session timers
        if (this.adminSessionTimeout) {
            clearTimeout(this.adminSessionTimeout);
        }
        if (this.adminSessionCheck) {
            clearInterval(this.adminSessionCheck);
        }

        // Clear admin data
        this.currentAdmin = null;

        // Cleanup auth manager
        if (this.auth && this.auth.cleanup) {
            this.auth.cleanup();
        }
    }
}

// Export AdminAuthentication class untuk global access
window.AdminAuthentication = AdminAuthentication;