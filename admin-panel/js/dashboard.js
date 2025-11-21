// /pdf-bookstore/admin-panel/js/dashboard.js

// Admin Dashboard Controller class untuk manage dashboard functionality
class AdminDashboard {
    constructor() {
        this.api = new AdminAPI(); // Admin API client dari admin-api.js line 3
        this.auth = new AdminAuthentication(); // Admin auth dari admin-auth.js line 3

        // Chart instances untuk proper cleanup
        this.charts = {
            sales: null,
            revenue: null,
            popularBooks: null
        };

        // Real-time update controls
        this.realTimeUpdates = null;
        this.refreshInterval = null;

        // Dashboard state management
        this.state = {
            isLoading: false,
            lastRefresh: null,
            autoRefresh: true,
            refreshRate: 30000 // 30 seconds
        };

        // Initialize dashboard
        this.initializeDashboard();
    }

    // Function untuk initialize dashboard dengan auth check dan data loading
    async initializeDashboard() {
        try {
            Utils.showLoading('Loading admin dashboard...');

            // Verify admin authentication sebelum load dashboard
            await this.verifyAdminAccess();

            // Setup event listeners untuk UI interactions
            this.setupEventListeners();

            // Load initial dashboard data
            await this.loadDashboardData();

            // Setup real-time updates
            this.setupRealTimeUpdates();

            // Setup auto-refresh
            this.setupAutoRefresh();

            Utils.hideLoading();
            Utils.showNotification('Dashboard loaded successfully', 'success');

        } catch (error) {
            console.error('Dashboard initialization failed:', error);
            Utils.hideLoading();

            if (error.message.includes('Admin')) {
                // Redirect ke login jika bukan admin
                Utils.showNotification('Admin access required. Redirecting to login...', 'error');
                setTimeout(() => {
                    window.location.href = 'index.html';
                }, 2000);
            } else {
                Utils.showNotification('Failed to load dashboard. Please refresh the page.', 'error');
            }
        }
    }

    // Function untuk verify admin access
    async verifyAdminAccess() {
        // Check authentication status dari AdminAuthentication
        if (!this.auth.isAdminAuthenticated()) {
            throw new Error('Admin authentication required');
        }

        // Get current admin user
        const currentAdmin = this.auth.getCurrentAdmin();
        if (!currentAdmin) {
            throw new Error('Admin user data not available');
        }

        // Update admin profile UI
        this.updateAdminProfile(currentAdmin);
    }

    // Function untuk load all dashboard data
    async loadDashboardData() {
        try {
            this.state.isLoading = true;

            // Load data dengan parallel requests untuk performance
            const [adminStats, recentOrders, topBooks, realTimeMetrics] = await Promise.all([
                this.api.getAdminStats(), // dari admin-api.js line 25
                this.api.getRecentOrders(5), // 5 recent orders untuk dashboard
                this.api.getTopBooks('sales', 5), // top 5 books by sales
                this.api.getRealTimeMetrics() // real-time metrics
            ]);

            // Update dashboard dengan loaded data
            this.updateStatistics(adminStats);
            this.updateRecentOrders(recentOrders);
            this.updateTopBooks(topBooks);
            this.updateRealTimeMetrics(realTimeMetrics);

            // Load dan render charts
            await this.loadCharts();

            // Load activity feed
            await this.loadActivityFeed();

            // Update system status
            await this.updateSystemStatus(adminStats.system);

            this.state.lastRefresh = new Date();

        } catch (error) {
            console.error('Failed to load dashboard data:', error);
            Utils.showNotification('Some dashboard data failed to load', 'warning');
        } finally {
            this.state.isLoading = false;
        }
    }

    // Function untuk update statistics cards
    updateStatistics(stats) {
        // Update books statistics
        const totalBooksEl = document.getElementById('total-books');
        const booksChangeEl = document.getElementById('books-change');

        if (totalBooksEl) {
            this.animateNumber(totalBooksEl, stats.books.total);
        }
        if (booksChangeEl) {
            this.updateChangeIndicator(booksChangeEl, stats.books.monthly_change, 'books');
        }

        // Update orders statistics
        const totalOrdersEl = document.getElementById('total-orders');
        const ordersChangeEl = document.getElementById('orders-change');

        if (totalOrdersEl) {
            this.animateNumber(totalOrdersEl, stats.orders.total);
        }
        if (ordersChangeEl) {
            this.updateChangeIndicator(ordersChangeEl, stats.orders.monthly_change, 'orders');
        }

        // Update revenue statistics
        const totalRevenueEl = document.getElementById('total-revenue');
        const revenueChangeEl = document.getElementById('revenue-change');

        if (totalRevenueEl) {
            this.animateNumber(totalRevenueEl, stats.revenue.total, true); // format as currency
        }
        if (revenueChangeEl) {
            this.updateChangeIndicator(revenueChangeEl, stats.revenue.monthly_change, 'revenue');
        }

        // Update users statistics
        const totalUsersEl = document.getElementById('total-users');
        const usersChangeEl = document.getElementById('users-change');

        if (totalUsersEl) {
            this.animateNumber(totalUsersEl, stats.users.total);
        }
        if (usersChangeEl) {
            this.updateChangeIndicator(usersChangeEl, stats.users.monthly_change, 'users');
        }

        // Update pending orders badge
        const pendingBadge = document.getElementById('pending-orders-badge');
        if (pendingBadge) {
            pendingBadge.textContent = stats.orders.pending;
            pendingBadge.style.display = stats.orders.pending > 0 ? 'inline' : 'none';
        }
    }

    // Function untuk animate number changes dengan smooth transition
    animateNumber(element, targetValue, isCurrency = false) {
        const currentValue = parseInt(element.textContent.replace(/[^0-9]/g, '')) || 0;
        const difference = targetValue - currentValue;
        const duration = 1000; // 1 second animation
        const steps = 60; // 60 FPS
        const stepValue = difference / steps;
        const stepTime = duration / steps;

        let currentStep = 0;

        const interval = setInterval(() => {
            currentStep++;
            const newValue = currentValue + (stepValue * currentStep);

            if (isCurrency) {
                element.textContent = Utils.formatCurrency(Math.round(newValue));
            } else {
                element.textContent = Math.round(newValue).toLocaleString();
            }

            if (currentStep >= steps) {
                clearInterval(interval);
                // Set final value untuk ensure accuracy
                if (isCurrency) {
                    element.textContent = Utils.formatCurrency(targetValue);
                } else {
                    element.textContent = targetValue.toLocaleString();
                }
            }
        }, stepTime);
    }

    // Function untuk update change indicators dengan proper styling
    updateChangeIndicator(element, changeValue, type) {
        const isPositive = changeValue >= 0;
        const icon = isPositive ? 'fa-arrow-up' : 'fa-arrow-down';
        const className = isPositive ? 'positive' : 'negative';
        const sign = isPositive ? '+' : '';

        // Update element content dan styling
        element.className = `stat-change ${className}`;
        element.innerHTML = `
            <i class="fas ${icon}"></i>
            ${sign}${Math.abs(changeValue).toFixed(1)}% from last month
        `;

        // Add animation class
        element.classList.add('fade-in');
    }

    // Function untuk update recent orders list
    updateRecentOrders(orders) {
        const recentOrdersContainer = document.getElementById('recent-orders');

        if (!recentOrdersContainer) return;

        if (!orders || orders.length === 0) {
            recentOrdersContainer.innerHTML = `
                <div class="empty-state">
                    <i class="fas fa-shopping-cart"></i>
                    <p>No recent orders</p>
                </div>
            `;
            return;
        }

        // Render recent orders dengan proper formatting
        recentOrdersContainer.innerHTML = orders.map(order => `
            <div class="order-item" data-order-id="${order.order.id}">
                <div class="order-info">
                    <div class="order-number">${order.order.order_number}</div>
                    <div class="order-customer">${order.book_title || 'Unknown Book'}</div>
                </div>
                <div class="order-amount">${Utils.formatCurrency(parseFloat(order.order.amount))}</div>
            </div>
        `).join('');
    }

    // Function untuk update top books list
    updateTopBooks(books) {
        const topBooksContainer = document.getElementById('top-books-list');

        if (!topBooksContainer) return;

        if (!books || books.length === 0) {
            topBooksContainer.innerHTML = `
                <div class="empty-state">
                    <i class="fas fa-book"></i>
                    <p>No books data available</p>
                </div>
            `;
            return;
        }

        // Render top books dengan ranking
        topBooksContainer.innerHTML = books.map((book, index) => `
            <div class="top-book-item" data-book-id="${book.id}">
                <div class="book-rank">${index + 1}</div>
                <div class="book-info">
                    <div class="book-title">${Utils.escapeHtml(book.title.substring(0, 30))}${book.title.length > 30 ? '...' : ''}</div>
                    <div class="book-author">by ${Utils.escapeHtml(book.author)}</div>
                </div>
                <div class="book-metric">${book.sales_count || 0} sales</div>
            </div>
        `).join('');
    }

    // Function untuk update real-time metrics
    updateRealTimeMetrics(metrics) {
        // Update online users
        const onlineUsersEl = document.getElementById('online-users');
        if (onlineUsersEl) {
            onlineUsersEl.textContent = metrics.online_users || '--';
        }

        // Update today's sales
        const todaySalesEl = document.getElementById('today-sales');
        if (todaySalesEl) {
            todaySalesEl.textContent = Utils.formatCurrency(metrics.today_sales || 0);
        }
    }

    // Function untuk load dan render all charts
    async loadCharts() {
        try {
            // Load sales chart
            await this.loadSalesChart();

            // Load revenue chart
            await this.loadRevenueChart();

            // Load popular books chart
            await this.loadPopularBooksChart();

        } catch (error) {
            console.error('Failed to load charts:', error);
            Utils.showNotification('Some charts failed to load', 'warning');
        }
    }

    // Function untuk load sales chart
    async loadSalesChart() {
        try {
            const chartData = await this.api.getSalesChartData(30); // 30 days data
            const ctx = document.getElementById('sales-chart');

            if (!ctx) return;

            // Destroy existing chart jika ada
            if (this.charts.sales) {
                this.charts.sales.destroy();
            }

            // Create new sales chart dengan Chart.js
            this.charts.sales = new Chart(ctx, {
                type: 'line',
                data: chartData,
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: {
                        legend: {
                            display: false
                        }
                    },
                    scales: {
                        y: {
                            beginAtZero: true,
                            grid: {
                                color: 'rgba(148, 163, 184, 0.1)'
                            },
                            ticks: {
                                color: '#94a3b8'
                            }
                        },
                        x: {
                            grid: {
                                display: false
                            },
                            ticks: {
                                color: '#94a3b8'
                            }
                        }
                    },
                    elements: {
                        point: {
                            radius: 4,
                            hoverRadius: 6
                        }
                    }
                }
            });

        } catch (error) {
            console.error('Failed to load sales chart:', error);
        }
    }

    // Function untuk load revenue chart
    async loadRevenueChart() {
        try {
            const chartData = await this.api.getRevenueChartData('monthly');
            const ctx = document.getElementById('revenue-chart');

            if (!ctx) return;

            if (this.charts.revenue) {
                this.charts.revenue.destroy();
            }

            this.charts.revenue = new Chart(ctx, {
                type: 'line',
                data: chartData,
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    interaction: {
                        mode: 'index',
                        intersect: false,
                    },
                    plugins: {
                        legend: {
                            display: true,
                            position: 'top',
                            labels: {
                                color: '#cbd5e1',
                                usePointStyle: true,
                                padding: 20
                            }
                        }
                    },
                    scales: {
                        y: {
                            type: 'linear',
                            display: true,
                            position: 'left',
                            grid: {
                                color: 'rgba(148, 163, 184, 0.1)'
                            },
                            ticks: {
                                color: '#94a3b8',
                                callback: function(value) {
                                    return Utils.formatCurrency(value);
                                }
                            }
                        },
                        y1: {
                            type: 'linear',
                            display: true,
                            position: 'right',
                            grid: {
                                drawOnChartArea: false,
                            },
                            ticks: {
                                color: '#94a3b8'
                            }
                        },
                        x: {
                            grid: {
                                display: false
                            },
                            ticks: {
                                color: '#94a3b8'
                            }
                        }
                    }
                }
            });

        } catch (error) {
            console.error('Failed to load revenue chart:', error);
        }
    }

    // Function untuk load popular books chart
    async loadPopularBooksChart() {
        try {
            const chartData = await this.api.getPopularBooksChartData();
            const ctx = document.getElementById('popular-books-chart');

            if (!ctx) return;

            if (this.charts.popularBooks) {
                this.charts.popularBooks.destroy();
            }

            this.charts.popularBooks = new Chart(ctx, {
                type: 'doughnut',
                data: chartData,
                options: {
                    responsive: true,
                    maintainAspectRatio: false,
                    plugins: {
                        legend: {
                            display: true,
                            position: 'bottom',
                            labels: {
                                color: '#cbd5e1',
                                padding: 15,
                                font: {
                                    size: 11
                                }
                            }
                        }
                    }
                }
            });

        } catch (error) {
            console.error('Failed to load popular books chart:', error);
        }
    }

    // Function untuk load activity feed
    async loadActivityFeed() {
        try {
            const activities = await this.api.getActivityFeed('all', 10);
            const activityFeedContainer = document.getElementById('activity-feed');

            if (!activityFeedContainer) return;

            if (!activities || activities.length === 0) {
                activityFeedContainer.innerHTML = `
                    <div class="empty-state">
                        <i class="fas fa-clock"></i>
                        <p>No recent activities</p>
                    </div>
                `;
                return;
            }

            // Render activity items dengan proper formatting
            activityFeedContainer.innerHTML = activities.map(activity => `
                <div class="activity-item">
                    <div class="activity-avatar">
                        <i class="fas fa-${activity.icon || 'circle'}"></i>
                    </div>
                    <div class="activity-details">
                        <p class="activity-text">${Utils.escapeHtml(activity.text)}</p>
                        <p class="activity-time">${Utils.formatRelativeTime(activity.timestamp)}</p>
                    </div>
                </div>
            `).join('');

        } catch (error) {
            console.error('Failed to load activity feed:', error);
        }
    }

    // Function untuk update system status indicators
    async updateSystemStatus(systemData) {
        if (!systemData || !systemData.services_status) return;

        const services = systemData.services_status;

        // Update individual service status
        Object.keys(services).forEach(serviceName => {
            const service = services[serviceName];
            const statusElement = document.getElementById(`${serviceName.replace('-', '')}-status`);

            if (statusElement) {
                const isOnline = service.status === 'online';
                const statusClass = isOnline ? 'status-online' : 'status-offline';
                const statusText = isOnline ? 'Online' : 'Offline';

                statusElement.innerHTML = `
                    <i class="fas fa-circle ${statusClass}"></i>
                    ${statusText}
                `;
            }
        });

        // Update overall system status
        const systemStatusElement = document.getElementById('system-status');
        if (systemStatusElement) {
            const allOnline = Object.values(services).every(service => service.status === 'online');
            const statusClass = allOnline ? 'status-online' : 'status-offline';
            const statusText = allOnline ? 'All systems operational' : 'Some services offline';

            systemStatusElement.innerHTML = `
                <span class="status-indicator ${statusClass}"></span>
                <span class="status-text">${statusText}</span>
            `;
        }
    }

    // Function untuk setup event listeners
    setupEventListeners() {
        // Sidebar toggle
        this.setupSidebarToggle();

        // Chart controls
        this.setupChartControls();

        // Refresh controls
        this.setupRefreshControls();

        // Navigation events
        this.setupNavigationEvents();

        // Quick actions
        this.setupQuickActions();

        // Mobile menu
        this.setupMobileMenu();
    }

    // Function untuk setup sidebar toggle functionality
    setupSidebarToggle() {
        const sidebarToggle = document.getElementById('sidebar-toggle');
        const sidebar = document.getElementById('admin-sidebar');

        if (sidebarToggle && sidebar) {
            sidebarToggle.addEventListener('click', () => {
                sidebar.classList.toggle('collapsed');

                // Save sidebar state ke localStorage
                localStorage.setItem('admin_sidebar_collapsed', sidebar.classList.contains('collapsed'));
            });

            // Restore sidebar state dari localStorage
            const isCollapsed = localStorage.getItem('admin_sidebar_collapsed') === 'true';
            if (isCollapsed) {
                sidebar.classList.add('collapsed');
            }
        }
    }

    // Function untuk setup chart controls
    setupChartControls() {
        // Sales period selector
        const salesPeriodSelect = document.getElementById('sales-period');
        if (salesPeriodSelect) {
            salesPeriodSelect.addEventListener('change', async(e) => {
                await this.updateSalesChart(e.target.value);
            });
        }

        // Revenue period selector
        const revenuePeriodSelect = document.getElementById('revenue-period');
        if (revenuePeriodSelect) {
            revenuePeriodSelect.addEventListener('change', async(e) => {
                await this.updateRevenueChart(e.target.value);
            });
        }

        // Top books metric selector
        const topBooksMetricSelect = document.getElementById('top-books-metric');
        if (topBooksMetricSelect) {
            topBooksMetricSelect.addEventListener('change', async(e) => {
                await this.updateTopBooksDisplay(e.target.value);
            });
        }

        // Chart refresh buttons
        const refreshButtons = document.querySelectorAll('.chart-refresh');
        refreshButtons.forEach(button => {
            button.addEventListener('click', async() => {
                button.disabled = true;
                button.innerHTML = '<i class="fas fa-spinner fa-spin"></i>';

                await this.loadCharts();

                button.disabled = false;
                button.innerHTML = '<i class="fas fa-refresh"></i>';
            });
        });
    }

    // Function untuk setup refresh controls
    setupRefreshControls() {
        // Activity feed refresh
        const refreshActivityBtn = document.getElementById('refresh-activity');
        if (refreshActivityBtn) {
            refreshActivityBtn.addEventListener('click', async() => {
                refreshActivityBtn.disabled = true;
                refreshActivityBtn.innerHTML = '<i class="fas fa-spinner fa-spin"></i> Refreshing...';

                await this.loadActivityFeed();

                refreshActivityBtn.disabled = false;
                refreshActivityBtn.innerHTML = '<i class="fas fa-refresh"></i> Refresh';
            });
        }

        // Activity filter
        const activityFilter = document.getElementById('activity-filter');
        if (activityFilter) {
            activityFilter.addEventListener('change', async(e) => {
                const activities = await this.api.getActivityFeed(e.target.value, 10);
                this.updateActivityFeed(activities);
            });
        }

        // Popular books refresh
        const refreshPopularBooks = document.getElementById('refresh-popular-books');
        if (refreshPopularBooks) {
            refreshPopularBooks.addEventListener('click', async() => {
                await this.loadPopularBooksChart();
            });
        }
    }

    // Function untuk setup navigation events
    setupNavigationEvents() {
        // Logout functionality
        const logoutBtn = document.getElementById('admin-logout');
        if (logoutBtn) {
            logoutBtn.addEventListener('click', async(e) => {
                e.preventDefault();
                await this.handleLogout();
            });
        }

        // Navigation links untuk proper active state
        const navLinks = document.querySelectorAll('.nav-link');
        navLinks.forEach(link => {
            link.addEventListener('click', (e) => {
                // Update active state
                navLinks.forEach(l => l.classList.remove('active'));
                link.classList.add('active');
            });
        });
    }

    // Function untuk setup quick actions
    setupQuickActions() {
        const quickActions = document.querySelectorAll('.quick-action');
        quickActions.forEach(action => {
            action.addEventListener('click', (e) => {
                // Add loading state untuk visual feedback
                const actionIcon = action.querySelector('.action-icon i');
                if (actionIcon) {
                    const originalClass = actionIcon.className;
                    actionIcon.className = 'fas fa-spinner fa-spin';

                    // Reset after navigation
                    setTimeout(() => {
                        actionIcon.className = originalClass;
                    }, 1000);
                }
            });
        });
    }

    // Function untuk setup mobile menu
    setupMobileMenu() {
        const mobileMenuToggle = document.getElementById('mobile-menu-toggle');
        const sidebar = document.getElementById('admin-sidebar');

        if (mobileMenuToggle && sidebar) {
            mobileMenuToggle.addEventListener('click', () => {
                sidebar.classList.toggle('mobile-open');
            });

            // Close mobile menu saat click outside
            document.addEventListener('click', (e) => {
                if (!sidebar.contains(e.target) && !mobileMenuToggle.contains(e.target)) {
                    sidebar.classList.remove('mobile-open');
                }
            });
        }
    }

    // Function untuk update sales chart dengan new period
    async updateSalesChart(period) {
        try {
            const chartData = await this.api.getSalesChartData(period);

            if (this.charts.sales) {
                this.charts.sales.data = chartData;
                this.charts.sales.update('active');
            }
        } catch (error) {
            console.error('Failed to update sales chart:', error);
            Utils.showNotification('Failed to update sales chart', 'error');
        }
    }

    // Function untuk update revenue chart dengan new period
    async updateRevenueChart(period) {
        try {
            const chartData = await this.api.getRevenueChartData(period);

            if (this.charts.revenue) {
                this.charts.revenue.data = chartData;
                this.charts.revenue.update('active');
            }
        } catch (error) {
            console.error('Failed to update revenue chart:', error);
            Utils.showNotification('Failed to update revenue chart', 'error');
        }
    }

    // Function untuk update top books display dengan new metric
    async updateTopBooksDisplay(metric) {
        try {
            const topBooks = await this.api.getTopBooks(metric, 5);
            this.updateTopBooks(topBooks);
        } catch (error) {
            console.error('Failed to update top books:', error);
            Utils.showNotification('Failed to update top books', 'error');
        }
    }

    // Function untuk update activity feed
    updateActivityFeed(activities) {
        const activityFeedContainer = document.getElementById('activity-feed');

        if (!activityFeedContainer) return;

        if (!activities || activities.length === 0) {
            activityFeedContainer.innerHTML = `
                <div class="empty-state">
                    <i class="fas fa-clock"></i>
                    <p>No activities found</p>
                </div>
            `;
            return;
        }

        activityFeedContainer.innerHTML = activities.map(activity => `
            <div class="activity-item fade-in">
                <div class="activity-avatar">
                    <i class="fas fa-${activity.icon || 'circle'}"></i>
                </div>
                <div class="activity-details">
                    <p class="activity-text">${Utils.escapeHtml(activity.text)}</p>
                    <p class="activity-time">${Utils.formatRelativeTime(activity.timestamp)}</p>
                </div>
            </div>
        `).join('');
    }

    // Function untuk setup real-time updates
    setupRealTimeUpdates() {
        if (this.realTimeUpdates) {
            this.realTimeUpdates(); // cleanup existing
        }

        // Setup real-time updates dengan AdminAPI
        this.realTimeUpdates = this.api.setupRealTimeUpdates((eventType, data) => {
            switch (eventType) {
                case 'metrics_update':
                    this.updateRealTimeMetrics(data);
                    break;
                default:
                    console.log('Real-time event:', eventType, data);
            }
        });
    }

    // Function untuk setup auto-refresh functionality
    setupAutoRefresh() {
        if (this.state.autoRefresh) {
            this.refreshInterval = setInterval(async() => {
                if (!this.state.isLoading) {
                    await this.refreshDashboardData();
                }
            }, this.state.refreshRate);
        }
    }

    // Function untuk refresh dashboard data
    async refreshDashboardData() {
        try {
            // Get latest metrics tanpa full reload
            const realTimeMetrics = await this.api.getRealTimeMetrics();
            this.updateRealTimeMetrics(realTimeMetrics);

            // Update timestamp
            this.state.lastRefresh = new Date();

            // Optional: Show refresh indicator
            const refreshIndicator = document.createElement('div');
            refreshIndicator.className = 'refresh-indicator';
            refreshIndicator.innerHTML = '<i class="fas fa-sync-alt fa-spin"></i>';
            refreshIndicator.style.cssText = `
                position: fixed;
                top: 20px;
                right: 20px;
                background: var(--admin-primary);
                color: white;
                padding: 8px 12px;
                border-radius: 6px;
                font-size: 12px;
                z-index: 10001;
            `;

            document.body.appendChild(refreshIndicator);

            setTimeout(() => {
                refreshIndicator.remove();
            }, 1000);

        } catch (error) {
            console.warn('Dashboard refresh failed:', error);
        }
    }

    // Function untuk handle admin logout
    async handleLogout() {
        try {
            // Confirm logout action
            if (!confirm('Are you sure you want to logout from admin panel?')) {
                return;
            }

            Utils.showLoading('Logging out...');

            // Cleanup resources
            this.cleanup();

            // Logout menggunakan admin auth
            await this.auth.logout();

            // Redirect ke login page
            window.location.href = 'index.html';

        } catch (error) {
            console.error('Logout failed:', error);
            Utils.hideLoading();
            Utils.showNotification('Logout failed. Please try again.', 'error');
        }
    }

    // Function untuk update admin profile information
    updateAdminProfile(adminUser) {
        // Update admin name in header
        const adminNameEl = document.getElementById('admin-name');
        if (adminNameEl) {
            adminNameEl.textContent = adminUser.full_name || 'Admin';
        }

        // Update profile sections jika ada
        const profileElements = document.querySelectorAll('.admin-profile-name');
        profileElements.forEach(el => {
            el.textContent = adminUser.full_name || 'Admin';
        });

        const profileEmails = document.querySelectorAll('.admin-profile-email');
        profileEmails.forEach(el => {
            el.textContent = adminUser.email || '';
        });
    }

    // Function untuk show dashboard loading states
    showLoadingStates() {
        // Add loading skeletons untuk stat cards
        const statNumbers = document.querySelectorAll('.stat-number');
        statNumbers.forEach(el => {
            el.classList.add('loading-skeleton');
            el.textContent = '--';
        });

        // Add loading state untuk charts
        const chartContainers = document.querySelectorAll('.chart-container');
        chartContainers.forEach(container => {
            container.classList.add('loading');
        });
    }

    // Function untuk hide dashboard loading states
    hideLoadingStates() {
        // Remove loading skeletons
        const loadingElements = document.querySelectorAll('.loading-skeleton');
        loadingElements.forEach(el => {
            el.classList.remove('loading-skeleton');
        });

        // Remove chart loading states
        const chartContainers = document.querySelectorAll('.chart-container');
        chartContainers.forEach(container => {
            container.classList.remove('loading');
        });
    }

    // Function untuk export dashboard data
    async exportDashboardData(format = 'json') {
        try {
            Utils.showLoading('Preparing export...');

            // Validate admin permissions untuk export
            await this.api.validateAdminPermissions('export_dashboard');

            // Export data menggunakan AdminAPI
            const exportData = await this.api.exportAdminData('all', format);

            // Create download link
            const blob = new Blob([exportData], {
                type: format === 'csv' ? 'text/csv' : 'application/json'
            });

            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `admin_dashboard_export_${new Date().toISOString().split('T')[0]}.${format}`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            window.URL.revokeObjectURL(url);

            Utils.showNotification('Dashboard data exported successfully', 'success');

        } catch (error) {
            console.error('Export failed:', error);
            Utils.showNotification('Export failed: ' + error.message, 'error');
        } finally {
            Utils.hideLoading();
        }
    }

    // Function untuk get dashboard performance metrics
    getDashboardMetrics() {
        return {
            load_time: this.state.lastRefresh ? Date.now() - this.state.lastRefresh.getTime() : 0,
            charts_loaded: Object.values(this.charts).filter(chart => chart !== null).length,
            auto_refresh_enabled: this.state.autoRefresh,
            refresh_rate: this.state.refreshRate,
            cache_stats: this.api.getCacheStats()
        };
    }

    // Function untuk cleanup dashboard resources
    cleanup() {
        // Destroy all charts
        Object.values(this.charts).forEach(chart => {
            if (chart) {
                chart.destroy();
            }
        });

        // Clear intervals
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
        }

        // Cleanup real-time updates
        if (this.realTimeUpdates) {
            this.realTimeUpdates();
        }

        // Cleanup API resources
        if (this.api && this.api.cleanup) {
            this.api.cleanup();
        }

        // Cleanup auth resources
        if (this.auth && this.auth.cleanup) {
            this.auth.cleanup();
        }

        console.log('Dashboard cleanup completed');
    }

    // Function untuk handle window events
    handleWindowEvents() {
        // Handle page visibility change untuk pause/resume updates
        document.addEventListener('visibilitychange', () => {
            if (document.hidden) {
                // Pause updates saat tab tidak aktif
                if (this.refreshInterval) {
                    clearInterval(this.refreshInterval);
                }
            } else {
                // Resume updates saat tab aktif kembali
                if (this.state.autoRefresh) {
                    this.setupAutoRefresh();
                }
            }
        });

        // Handle window resize untuk responsive charts
        window.addEventListener('resize', Utils.debounce(() => {
            Object.values(this.charts).forEach(chart => {
                if (chart) {
                    chart.resize();
                }
            });
        }, 250));

        // Handle beforeunload untuk cleanup
        window.addEventListener('beforeunload', () => {
            this.cleanup();
        });
    }
}

// Initialize window events saat class di-load
document.addEventListener('DOMContentLoaded', () => {
    // Setup global error handling untuk dashboard
    window.addEventListener('error', (event) => {
        console.error('Dashboard error:', event.error);

        if (window.adminDashboard) {
            Utils.showNotification('An error occurred. Dashboard data may be outdated.', 'warning');
        }
    });
});

// Export AdminDashboard class untuk global access
window.AdminDashboard = AdminDashboard;