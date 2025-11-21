// /pdf-bookstore/admin-panel/js/order-manager.js

// Order Manager class untuk handle complete order management dengan analytics
class OrderManager {
    constructor() {
        this.api = new AdminAPI(); // Admin API client dari admin-api.js line 3
        this.auth = new AdminAuthentication(); // Admin auth dari admin-auth.js line 3

        // Order management state
        this.state = {
            orders: [],
            filteredOrders: [],
            currentPage: 1,
            itemsPerPage: 25,
            totalOrders: 0,
            sortBy: 'created_at',
            sortOrder: 'desc',
            filters: {
                search: '',
                status: '',
                payment_method: ''
            },
            statistics: {},
            isLoading: false,
            lastRefresh: null
        };

        // Chart instances untuk proper cleanup
        this.charts = {
            revenue: null
        };

        // Real-time update controls
        this.realTimeUpdates = null;
        this.refreshInterval = null;

        // Order detail modal state
        this.orderModal = {
            isOpen: false,
            currentOrder: null
        };

        // Initialize order manager
        this.initializeOrderManager();
    }

    // Function untuk initialize order manager dengan auth check dan data loading
    async initializeOrderManager() {
        try {
            // Verify admin access sebelum load order management
            await this.verifyAdminAccess();

            // Setup event listeners untuk all interactions
            this.setupEventListeners();

            // Load initial data
            await this.loadInitialData();

            // Setup real-time updates untuk order monitoring
            this.setupRealTimeUpdates();

            // Setup auto-refresh untuk live data
            this.setupAutoRefresh();

            Utils.showNotification('Order management loaded successfully', 'success');

        } catch (error) {
            console.error('Order manager initialization failed:', error);
            Utils.showNotification('Failed to initialize order management', 'error');

            if (error.message.includes('Admin')) {
                setTimeout(() => {
                    window.location.href = 'index.html';
                }, 2000);
            }
        }
    }

    // Function untuk verify admin access
    async verifyAdminAccess() {
        if (!this.auth.isAdminAuthenticated()) {
            throw new Error('Admin authentication required');
        }

        const currentAdmin = this.auth.getCurrentAdmin();
        if (!currentAdmin) {
            throw new Error('Admin user data not available');
        }

        // Update admin profile
        this.updateAdminProfile(currentAdmin);
    }

    // Function untuk load initial data (orders, statistics, analytics)
    async loadInitialData() {
        try {
            Utils.showLoading('Loading order data...');

            // Load data dengan parallel requests untuk performance
            const [ordersData, orderStats, revenueAnalytics, realTimeMetrics] = await Promise.all([
                this.loadOrders(), // orders dengan pagination
                this.api.getAdminOrderStats(), // order statistics
                this.api.getRevenueAnalytics('monthly', 30), // revenue data untuk chart
                this.api.getRealTimeMetrics() // real-time metrics
            ]);

            // Update UI dengan loaded data
            this.updateOrderStatistics(orderStats);
            this.updateRealTimeMetrics(realTimeMetrics);

            // Load analytics charts
            await this.loadRevenueChart(revenueAnalytics);

            // Load additional analytics
            await this.loadPaymentMethodAnalytics();
            await this.loadRecentActivities();

            Utils.hideLoading();

        } catch (error) {
            Utils.hideLoading();
            console.error('Failed to load initial data:', error);
            Utils.showNotification('Some data failed to load', 'warning');
        }
    }

    // Function untuk load orders dengan pagination dan filtering
    async loadOrders() {
        try {
            this.state.isLoading = true;
            this.showLoadingState();

            // Build query parameters
            const params = new URLSearchParams({
                page: this.state.currentPage.toString(),
                per_page: this.state.itemsPerPage.toString(),
                sort_by: this.state.sortBy,
                sort_order: this.state.sortOrder,
                ...this.state.filters
            });

            // Remove empty filter values
            for (const [key, value] of params.entries()) {
                if (!value) {
                    params.delete(key);
                }
            }

            const response = await this.api.request(`${this.api.endpoints.payments}/admin/orders?${params}`);
            // ^^^^^^^ = API call ke payment-service/handlers.rs line 123

            if (response.success && response.data) {
                this.state.orders = response.data.orders;
                this.state.totalOrders = response.data.total;

                // Update UI
                this.renderOrders();
                this.updatePagination();
                this.updateOrderCount();
                this.hideLoadingState();
            }

        } catch (error) {
            console.error('Failed to load orders:', error);
            this.hideLoadingState();
            this.showEmptyState();
            Utils.showNotification('Failed to load orders', 'error');
        } finally {
            this.state.isLoading = false;
        }
    }

    // Function untuk setup event listeners
    setupEventListeners() {
        // Search dan filtering
        this.setupSearchAndFiltering();

        // Sorting functionality
        this.setupSorting();

        // Pagination controls
        this.setupPagination();

        // Order actions
        this.setupOrderActions();

        // Chart controls
        this.setupChartControls();

        // Export functionality
        this.setupExportControls();

        // Modal events
        this.setupModalEvents();

        // General UI events
        this.setupUIEvents();
    }

    // Function untuk setup search dan filtering functionality
    setupSearchAndFiltering() {
        // Search input dengan debouncing
        const searchInput = document.getElementById('order-search');
        if (searchInput) {
            searchInput.addEventListener('input', Utils.debounce(() => {
                this.state.filters.search = searchInput.value.trim();
                this.state.currentPage = 1;
                this.loadOrders();
            }, 300));
        }

        // Search submit button
        const searchSubmit = document.getElementById('search-submit');
        if (searchSubmit) {
            searchSubmit.addEventListener('click', () => {
                this.loadOrders();
            });
        }

        // Search clear button
        const searchClear = document.getElementById('search-clear');
        if (searchClear) {
            searchClear.addEventListener('click', () => {
                if (searchInput) searchInput.value = '';
                this.state.filters.search = '';
                this.state.currentPage = 1;
                this.loadOrders();
                searchClear.style.display = 'none';
            });
        }

        // Status filter
        const statusFilter = document.getElementById('status-filter');
        if (statusFilter) {
            statusFilter.addEventListener('change', () => {
                this.state.filters.status = statusFilter.value;
                this.state.currentPage = 1;
                this.loadOrders();
            });
        }

        // Payment method filter
        const paymentMethodFilter = document.getElementById('payment-method-filter');
        if (paymentMethodFilter) {
            paymentMethodFilter.addEventListener('change', () => {
                this.state.filters.payment_method = paymentMethodFilter.value;
                this.state.currentPage = 1;
                this.loadOrders();
            });
        }

        // Items per page selector
        const itemsPerPage = document.getElementById('items-per-page');
        if (itemsPerPage) {
            itemsPerPage.addEventListener('change', () => {
                this.state.itemsPerPage = parseInt(itemsPerPage.value);
                this.state.currentPage = 1;
                this.loadOrders();
            });
        }

        // Clear filters button
        const clearFilters = document.getElementById('clear-filters');
        if (clearFilters) {
            clearFilters.addEventListener('click', () => {
                this.clearAllFilters();
            });
        }

        // Show search clear button when search has value
        if (searchInput) {
            searchInput.addEventListener('input', () => {
                if (searchClear) {
                    searchClear.style.display = searchInput.value ? 'block' : 'none';
                }
            });
        }
    }

    // Function untuk setup sorting functionality
    setupSorting() {
        const sortableHeaders = document.querySelectorAll('.sortable');

        sortableHeaders.forEach(header => {
            header.addEventListener('click', () => {
                const sortField = header.dataset.sort;

                // Toggle sort order jika same field
                if (this.state.sortBy === sortField) {
                    this.state.sortOrder = this.state.sortOrder === 'asc' ? 'desc' : 'asc';
                } else {
                    this.state.sortBy = sortField;
                    this.state.sortOrder = 'asc';
                }

                // Update visual indicators
                this.updateSortIndicators();

                // Reload orders dengan new sorting
                this.state.currentPage = 1;
                this.loadOrders();
            });
        });
    }

    // Function untuk setup order actions
    setupOrderActions() {
        // Event delegation untuk order actions
        document.addEventListener('click', async(e) => {
            if (e.target.matches('.btn-view-order') || e.target.closest('.btn-view-order')) {
                const orderId = e.target.dataset.orderId || e.target.closest('.btn-view-order').dataset.orderId;
                await this.openOrderModal(orderId);
            }

            if (e.target.matches('.btn-refresh-status') || e.target.closest('.btn-refresh-status')) {
                const orderId = e.target.dataset.orderId || e.target.closest('.btn-refresh-status').dataset.orderId;
                await this.refreshOrderStatus(orderId);
            }
        });

        // Refresh orders button
        const refreshBtn = document.getElementById('refresh-orders');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', async() => {
                refreshBtn.disabled = true;
                refreshBtn.innerHTML = '<i class="fas fa-spinner fa-spin"></i>';

                await this.loadOrders();

                refreshBtn.disabled = false;
                refreshBtn.innerHTML = '<i class="fas fa-refresh"></i>';
            });
        }
    }

    // Function untuk setup chart controls
    setupChartControls() {
        // Revenue chart period selector
        const revenuePeriod = document.getElementById('revenue-period');
        if (revenuePeriod) {
            revenuePeriod.addEventListener('change', async(e) => {
                await this.updateRevenueChart(e.target.value);
            });
        }

        // Revenue chart refresh button
        const refreshRevenueChart = document.getElementById('refresh-revenue-chart');
        if (refreshRevenueChart) {
            refreshRevenueChart.addEventListener('click', async() => {
                refreshRevenueChart.disabled = true;
                refreshRevenueChart.innerHTML = '<i class="fas fa-spinner fa-spin"></i>';

                await this.loadRevenueChart();

                refreshRevenueChart.disabled = false;
                refreshRevenueChart.innerHTML = '<i class="fas fa-refresh"></i>';
            });
        }

        // Analytics refresh buttons
        const refreshPaymentMethods = document.getElementById('refresh-payment-methods');
        if (refreshPaymentMethods) {
            refreshPaymentMethods.addEventListener('click', async() => {
                await this.loadPaymentMethodAnalytics();
            });
        }

        const refreshActivities = document.getElementById('refresh-activities');
        if (refreshActivities) {
            refreshActivities.addEventListener('click', async() => {
                await this.loadRecentActivities();
            });
        }
    }

    // Function untuk setup export controls
    setupExportControls() {
        const exportBtn = document.getElementById('export-orders');
        if (exportBtn) {
            exportBtn.addEventListener('click', async() => {
                await this.exportOrders();
            });
        }
    }

    // Function untuk setup modal events
    setupModalEvents() {
        // Order details modal close
        const modalClose = document.getElementById('modal-close');
        const modalCancel = document.getElementById('modal-cancel');

        if (modalClose) {
            modalClose.addEventListener('click', () => {
                this.closeOrderModal();
            });
        }

        if (modalCancel) {
            modalCancel.addEventListener('click', () => {
                this.closeOrderModal();
            });
        }

        // Refresh order status in modal
        const refreshOrderStatus = document.getElementById('refresh-order-status');
        if (refreshOrderStatus) {
            refreshOrderStatus.addEventListener('click', async() => {
                if (this.orderModal.currentOrder) {
                    await this.refreshOrderStatus(this.orderModal.currentOrder.id, true);
                }
            });
        }

        // Close modal saat click outside
        document.addEventListener('click', (e) => {
            if (e.target.matches('.modal')) {
                this.closeOrderModal();
            }
        });
    }

    // Function untuk setup general UI events
    setupUIEvents() {
        // Sidebar toggle
        const sidebarToggle = document.getElementById('sidebar-toggle');
        const sidebar = document.getElementById('admin-sidebar');

        if (sidebarToggle && sidebar) {
            sidebarToggle.addEventListener('click', () => {
                sidebar.classList.toggle('collapsed');
            });
        }

        // Mobile menu toggle
        const mobileMenuToggle = document.getElementById('mobile-menu-toggle');
        if (mobileMenuToggle && sidebar) {
            mobileMenuToggle.addEventListener('click', () => {
                sidebar.classList.toggle('mobile-open');
            });
        }

        // Logout functionality
        const logoutBtn = document.getElementById('admin-logout');
        if (logoutBtn) {
            logoutBtn.addEventListener('click', async(e) => {
                e.preventDefault();
                await this.handleLogout();
            });
        }
    }

    // Function untuk render orders table
    renderOrders() {
        const tableBody = document.getElementById('orders-table-body');
        if (!tableBody) return;

        if (this.state.orders.length === 0) {
            this.showEmptyState();
            return;
        }

        tableBody.innerHTML = this.state.orders.map(order => `
            <tr class="order-row" data-order-id="${order.id}">
                <td class="order-id">
                    <div class="order-number">
                        <strong>${order.order_number}</strong>
                        <small class="order-time">${Utils.formatRelativeTime(order.created_at)}</small>
                    </div>
                </td>
                <td class="order-customer">
                    <div class="customer-info">
                        <div class="customer-name">${Utils.escapeHtml(order.customer_name || 'Unknown')}</div>
                        <div class="customer-email">${Utils.escapeHtml(order.customer_email || '')}</div>
                    </div>
                </td>
                <td class="order-book">
                    <div class="book-info">
                        <div class="book-title">${Utils.escapeHtml(order.book_title || 'Unknown Book')}</div>
                        <div class="book-author">by ${Utils.escapeHtml(order.book_author || 'Unknown Author')}</div>
                    </div>
                </td>
                <td class="order-amount">
                    <span class="amount-value">${Utils.formatCurrency(parseFloat(order.amount))}</span>
                </td>
                <td class="order-payment-method">
                    <span class="payment-method-badge">${this.formatPaymentMethod(order.payment_method)}</span>
                </td>
                <td class="order-status">
                    <span class="status-badge status-${order.status}">
                        ${this.formatOrderStatus(order.status)}
                    </span>
                </td>
                <td class="order-created">
                    <div class="date-info">
                        <div class="date-main">${Utils.formatDate(order.created_at)}</div>
                        <div class="date-time">${Utils.formatTime(order.created_at)}</div>
                    </div>
                </td>
                <td class="order-actions">
                    <div class="action-buttons">
                        <button class="btn btn-sm btn-primary btn-view-order" 
                                data-order-id="${order.id}" 
                                title="View Details">
                            <i class="fas fa-eye"></i>
                        </button>
                        <button class="btn btn-sm btn-info btn-refresh-status" 
                                data-order-id="${order.id}" 
                                title="Refresh Status">
                            <i class="fas fa-sync-alt"></i>
                        </button>
                    </div>
                </td>
            </tr>
        `).join('');
    }

    // Function untuk format payment method display
    formatPaymentMethod(method) {
        const methods = {
            'credit_card': 'Credit Card',
            'bank_transfer': 'Bank Transfer',
            'e_wallet': 'E-Wallet',
            'qris': 'QRIS',
            'gopay': 'GoPay',
            'ovo': 'OVO',
            'dana': 'DANA'
        };

        return methods[method] || Utils.capitalize(method);
    }

    // Function untuk format order status display
    formatOrderStatus(status) {
        const statuses = {
            'pending': 'Pending',
            'paid': 'Paid',
            'failed': 'Failed',
            'cancelled': 'Cancelled',
            'expired': 'Expired',
            'refunded': 'Refunded'
        };

        return statuses[status] || Utils.capitalize(status);
    }

    // Function untuk open order details modal
    async openOrderModal(orderId) {
        try {
            // Get detailed order information
            const response = await this.api.request(`${this.api.endpoints.payments}/admin/orders/${orderId}`);
            // ^^^^^^^ = API call ke payment-service/handlers.rs line 198

            if (!response.success || !response.data) {
                throw new Error('Failed to load order details');
            }

            const order = response.data;

            // Populate modal dengan order details
            this.populateOrderModal(order);

            // Show modal
            const modal = document.getElementById('order-details-modal');
            if (modal) {
                modal.classList.add('show');
                modal.style.display = 'flex';
            }

            this.orderModal.isOpen = true;
            this.orderModal.currentOrder = order;

        } catch (error) {
            console.error('Failed to open order modal:', error);
            Utils.showNotification('Failed to load order details', 'error');
        }
    }

    // Function untuk populate order modal dengan data
    populateOrderModal(order) {
            const modalContent = document.getElementById('order-details-content');
            if (!modalContent) return;

            // Update modal title
            const modalTitle = document.getElementById('modal-title');
            if (modalTitle) {
                modalTitle.textContent = `Order ${order.order_number}`;
            }

            // Generate order details HTML
            modalContent.innerHTML = `
            <div class="order-details-grid">
                <!-- Order Information -->
                <div class="detail-section">
                    <h4 class="section-title">Order Information</h4>
                    <div class="detail-items">
                        <div class="detail-item">
                            <span class="detail-label">Order Number</span>
                            <span class="detail-value">${order.order_number}</span>
                        </div>
                        <div class="detail-item">
                            <span class="detail-label">Status</span>
                            <span class="detail-value">
                                <span class="status-badge status-${order.status}">
                                    ${this.formatOrderStatus(order.status)}
                                </span>
                            </span>
                        </div>
                        <div class="detail-item">
                            <span class="detail-label">Amount</span>
                            <span class="detail-value amount-large">${Utils.formatCurrency(parseFloat(order.amount))}</span>
                        </div>
                        <div class="detail-item">
                            <span class="detail-label">Created</span>
                            <span class="detail-value">${Utils.formatDateTime(order.created_at)}</span>
                        </div>
                        ${order.paid_at ? `
                            <div class="detail-item">
                                <span class="detail-label">Paid At</span>
                                <span class="detail-value">${Utils.formatDateTime(order.paid_at)}</span>
                            </div>
                        ` : ''}
                        ${order.expires_at ? `
                            <div class="detail-item">
                                <span class="detail-label">Expires At</span>
                                <span class="detail-value">${Utils.formatDateTime(order.expires_at)}</span>
                            </div>
                        ` : ''}
                    </div>
                </div>

                <!-- Customer Information -->
                <div class="detail-section">
                    <h4 class="section-title">Customer Information</h4>
                    <div class="detail-items">
                        <div class="detail-item">
                            <span class="detail-label">Name</span>
                            <span class="detail-value">${Utils.escapeHtml(order.customer_name || 'Unknown')}</span>
                        </div>
                        <div class="detail-item">
                            <span class="detail-label">Email</span>
                            <span class="detail-value">${Utils.escapeHtml(order.customer_email || 'Unknown')}</span>
                        </div>
                        <div class="detail-item">
                            <span class="detail-label">User ID</span>
                            <span class="detail-value">${order.user_id || 'N/A'}</span>
                        </div>
                    </div>
                </div>

                <!-- Book Information -->
                <div class="detail-section">
                    <h4 class="section-title">Book Information</h4>
                    <div class="book-detail">
                        ${order.book_cover ? `
                            <div class="book-cover-detail">
                                <img src="${order.book_cover}" alt="Book cover" class="book-cover-image">
                            </div>
                        ` : ''}
                        <div class="book-info-detail">
                            <h5 class="book-title-detail">${Utils.escapeHtml(order.book_title || 'Unknown Book')}</h5>
                            <p class="book-author-detail">by ${Utils.escapeHtml(order.book_author || 'Unknown Author')}</p>
                            <div class="detail-items">
                                <div class="detail-item">
                                    <span class="detail-label">Book ID</span>
                                    <span class="detail-value">${order.book_id}</span>
                                </div>
                                <div class="detail-item">
                                    <span class="detail-label">Price</span>
                                    <span class="detail-value">${Utils.formatCurrency(parseFloat(order.book_price || order.amount))}</span>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Payment Information -->
                <div class="detail-section">
                    <h4 class="section-title">Payment Information</h4>
                    <div class="detail-items">
                        <div class="detail-item">
                            <span class="detail-label">Payment Method</span>
                            <span class="detail-value">
                                <span class="payment-method-badge">${this.formatPaymentMethod(order.payment_method)}</span>
                            </span>
                        </div>
                        ${order.midtrans_order_id ? `
                            <div class="detail-item">
                                <span class="detail-label">Midtrans Order ID</span>
                                <span class="detail-value">${order.midtrans_order_id}</span>
                            </div>
                        ` : ''}
                        ${order.payment_url ? `
                            <div class="detail-item">
                                <span class="detail-label">Payment URL</span>
                                <span class="detail-value">
                                    <a href="${order.payment_url}" target="_blank" class="payment-link">
                                        Open Payment Page <i class="fas fa-external-link-alt"></i>
                                    </a>
                                </span>
                            </div>
                        ` : ''}
                    </div>
                </div>

                ${order.payment_logs && order.payment_logs.length > 0 ? `
                    <!-- Payment Logs -->
                    <div class="detail-section full-width">
                        <h4 class="section-title">Payment History</h4>
                        <div class="payment-logs">
                            ${order.payment_logs.map(log => `
                                <div class="payment-log-item">
                                    <div class="log-header">
                                        <span class="log-status">${log.transaction_status}</span>
                                        <span class="log-time">${Utils.formatDateTime(log.created_at)}</span>
                                    </div>
                                    <div class="log-details">
                                        ${log.payment_type ? `<div>Payment Type: ${log.payment_type}</div>` : ''}
                                        ${log.gross_amount ? `<div>Amount: ${Utils.formatCurrency(parseFloat(log.gross_amount))}</div>` : ''}
                                        ${log.fraud_status ? `<div>Fraud Status: ${log.fraud_status}</div>` : ''}
                                    </div>
                                </div>
                            `).join('')}
                        </div>
                    </div>
                ` : ''}
            </div>
        `;
    }

    // Function untuk close order modal
    closeOrderModal() {
        const modal = document.getElementById('order-details-modal');
        if (modal) {
            modal.classList.remove('show');
            modal.style.display = 'none';
        }

        this.orderModal.isOpen = false;
        this.orderModal.currentOrder = null;
    }

    // Function untuk refresh order status
    async refreshOrderStatus(orderId, updateModal = false) {
        try {
            // Call payment service untuk refresh status dari Midtrans
            const response = await this.api.request(`${this.api.endpoints.payments}/admin/orders/${orderId}/refresh-status`, {
                method: 'POST'
            });

            if (!response.success) {
                throw new Error(response.message || 'Failed to refresh order status');
            }

            // Update order dalam current list
            const orderIndex = this.state.orders.findIndex(order => order.id === orderId);
            if (orderIndex !== -1) {
                this.state.orders[orderIndex] = { ...this.state.orders[orderIndex], ...response.data };
                this.renderOrders();
            }

            // Update modal jika terbuka
            if (updateModal && this.orderModal.isOpen && this.orderModal.currentOrder.id === orderId) {
                this.orderModal.currentOrder = { ...this.orderModal.currentOrder, ...response.data };
                this.populateOrderModal(this.orderModal.currentOrder);
            }

            Utils.showNotification('Order status refreshed', 'success');

        } catch (error) {
            console.error('Failed to refresh order status:', error);
            Utils.showNotification('Failed to refresh order status: ' + error.message, 'error');
        }
    }

    // Function untuk load revenue chart
    async loadRevenueChart(revenueData = null) {
        try {
            // Load data jika tidak disediakan
            if (!revenueData) {
                const period = document.getElementById('revenue-period')?.value || '30';
                revenueData = await this.api.getRevenueAnalytics('daily', parseInt(period));
            }

            const ctx = document.getElementById('revenue-chart');
            if (!ctx) return;

            // Destroy existing chart jika ada
            if (this.charts.revenue) {
                this.charts.revenue.destroy();
            }

            // Transform data untuk Chart.js format
            const chartData = this.transformRevenueData(revenueData);

            // Create new revenue chart dengan Chart.js
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
                        },
                        tooltip: {
                            callbacks: {
                                label: function(context) {
                                    if (context.datasetIndex === 0) {
                                        return `Revenue: ${Utils.formatCurrency(context.parsed.y)}`;
                                    } else {
                                        return `Orders: ${context.parsed.y}`;
                                    }
                                }
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

    // Function untuk transform revenue data untuk Chart.js
    transformRevenueData(revenueData) {
        if (!revenueData || !revenueData.data || !revenueData.data.data_points) {
            return {
                labels: [],
                datasets: []
            };
        }

        const dataPoints = revenueData.data.data_points;

        return {
            labels: dataPoints.map(point => Utils.formatChartDate(point.date)),
            datasets: [
                {
                    label: 'Revenue',
                    data: dataPoints.map(point => parseFloat(point.revenue || 0)),
                    borderColor: 'rgb(99, 102, 241)',
                    backgroundColor: 'rgba(99, 102, 241, 0.1)',
                    fill: true,
                    tension: 0.4,
                    yAxisID: 'y'
                },
                {
                    label: 'Orders',
                    data: dataPoints.map(point => parseInt(point.orders_count || 0)),
                    borderColor: 'rgb(16, 185, 129)',
                    backgroundColor: 'rgba(16, 185, 129, 0.1)',
                    fill: false,
                    tension: 0.4,
                    yAxisID: 'y1'
                }
            ]
        };
    }

    // Function untuk update revenue chart dengan new period
    async updateRevenueChart(period) {
        try {
            const revenueData = await this.api.getRevenueAnalytics('daily', parseInt(period));
            await this.loadRevenueChart(revenueData);
        } catch (error) {
            console.error('Failed to update revenue chart:', error);
            Utils.showNotification('Failed to update revenue chart', 'error');
        }
    }

    // Function untuk load payment method analytics
    async loadPaymentMethodAnalytics() {
        try {
            const response = await this.api.getPaymentMethodAnalytics();
            const container = document.getElementById('payment-methods-list');

            if (!container) return;

            if (!response || !response.data || response.data.length === 0) {
                container.innerHTML = `
                    <div class="empty-state">
                        <i class="fas fa-credit-card"></i>
                        <p>No payment method data available</p>
                    </div>
                `;
                return;
            }

            // Render payment methods dengan percentage
            const total = response.data.reduce((sum, method) => sum + method.count, 0);

            container.innerHTML = response.data.map(method => {
                const percentage = total > 0 ? (method.count / total * 100).toFixed(1) : 0;
                
                return `
                    <div class="payment-method-item">
                        <div class="method-info">
                            <span class="method-name">${this.formatPaymentMethod(method.payment_method)}</span>
                            <span class="method-count">${method.count} orders</span>
                        </div>
                        <div class="method-stats">
                            <div class="progress-bar">
                                <div class="progress-fill" style="width: ${percentage}%"></div>
                            </div>
                            <span class="method-percentage">${percentage}%</span>
                        </div>
                    </div>
                `;
            }).join('');

        } catch (error) {
            console.error('Failed to load payment method analytics:', error);
        }
    }

    // Function untuk load recent activities
    async loadRecentActivities() {
        try {
            const response = await this.api.getAdminActivityFeed(10, 'orders');
            const container = document.getElementById('order-activities');

            if (!container) return;

            if (!response || !response.data || response.data.length === 0) {
                container.innerHTML = `
                    <div class="empty-state">
                        <i class="fas fa-clock"></i>
                        <p>No recent activities</p>
                    </div>
                `;
                return;
            }

            // Render activities
            container.innerHTML = response.data.map(activity => `
                <div class="activity-item">
                    <div class="activity-avatar">
                        <i class="fas fa-${this.getActivityIcon(activity.type)}"></i>
                    </div>
                    <div class="activity-details">
                        <p class="activity-text">${Utils.escapeHtml(activity.description)}</p>
                        <p class="activity-time">${Utils.formatRelativeTime(activity.created_at)}</p>
                    </div>
                </div>
            `).join('');

        } catch (error) {
            console.error('Failed to load recent activities:', error);
        }
    }

    // Function untuk get activity icon berdasarkan type
    getActivityIcon(activityType) {
        const icons = {
            'order_created': 'plus-circle',
            'order_paid': 'check-circle',
            'order_failed': 'times-circle',
            'order_refunded': 'undo',
            'payment_received': 'dollar-sign'
        };

        return icons[activityType] || 'circle';
    }

    // Function untuk update order statistics
    updateOrderStatistics(stats) {
        if (!stats || !stats.data) return;

        const data = stats.data;

        // Update stat cards
        const updates = {
            'total-orders-stat': data.total || 0,
            'pending-orders-stat': data.pending || 0,
            'total-revenue-stat': data.total_revenue || 0,
            'avg-order-value-stat': data.average_order_value || 0
        };

        Object.entries(updates).forEach(([id, value]) => {
            const element = document.getElementById(id);
            if (element) {
                if (id.includes('revenue') || id.includes('avg-order')) {
                    element.textContent = Utils.formatCurrency(value);
                } else {
                    element.textContent = value.toLocaleString();
                }
            }
        });

        // Update change indicators
        const changes = {
            'orders-change': data.monthly_change || 0,
            'pending-change': 'Awaiting payment',
            'revenue-change': data.revenue_change || 0,
            'aov-change': 'Per transaction'
        };

        Object.entries(changes).forEach(([id, value]) => {
            const element = document.getElementById(id);
            if (element) {
                if (typeof value === 'string') {
                    element.innerHTML = `<i class="fas fa-info-circle"></i> ${value}`;
                } else {
                    const icon = value >= 0 ? 'fa-arrow-up' : 'fa-arrow-down';
                    const className = value >= 0 ? 'positive' : 'negative';
                    element.className = `stat-change ${className}`;
                    element.innerHTML = `<i class="fas ${icon}"></i> ${Math.abs(value).toFixed(1)}% from last month`;
                }
            }
        });

        // Update pending orders badge
        const pendingBadge = document.getElementById('pending-orders-badge');
        if (pendingBadge) {
            pendingBadge.textContent = data.pending || 0;
            pendingBadge.style.display = (data.pending || 0) > 0 ? 'inline' : 'none';
        }
    }

    // Function untuk update real-time metrics
    updateRealTimeMetrics(metrics) {
        if (!metrics) return;

        // Update header stats
        const todayOrders = document.getElementById('header-today-orders');
        const todayRevenue = document.getElementById('header-today-revenue');

        if (todayOrders) {
            todayOrders.textContent = metrics.today_orders || '--';
        }

        if (todayRevenue) {
            todayRevenue.textContent = Utils.formatCurrency(metrics.today_revenue || 0);
        }
    }

    // Function untuk update pagination
    updatePagination() {
        const totalPages = Math.ceil(this.state.totalOrders / this.state.itemsPerPage);
        const paginationControls = document.getElementById('pagination-controls');

        if (!paginationControls || totalPages <= 1) {
            if (paginationControls) paginationControls.innerHTML = '';
            return;
        }

        // Generate pagination HTML
        let paginationHTML = '';

        // Previous button
        if (this.state.currentPage > 1) {
            paginationHTML += `
                <button class="pagination-btn" data-page="${this.state.currentPage - 1}">
                    <i class="fas fa-chevron-left"></i> Previous
                </button>
            `;
        }

        // Page numbers
        const startPage = Math.max(1, this.state.currentPage - 2);
        const endPage = Math.min(totalPages, this.state.currentPage + 2);

        if (startPage > 1) {
            paginationHTML += `<button class="pagination-btn" data-page="1">1</button>`;
            if (startPage > 2) {
                paginationHTML += `<span class="pagination-ellipsis">...</span>`;
            }
        }

        for (let i = startPage; i <= endPage; i++) {
            paginationHTML += `
                <button class="pagination-btn ${i === this.state.currentPage ? 'active' : ''}" 
                        data-page="${i}">
                    ${i}
                </button>
            `;
        }

        if (endPage < totalPages) {
            if (endPage < totalPages - 1) {
                paginationHTML += `<span class="pagination-ellipsis">...</span>`;
            }
            paginationHTML += `<button class="pagination-btn" data-page="${totalPages}">${totalPages}</button>`;
        }

        // Next button
        if (this.state.currentPage < totalPages) {
            paginationHTML += `
                <button class="pagination-btn" data-page="${this.state.currentPage + 1}">
                    Next <i class="fas fa-chevron-right"></i>
                </button>
            `;
        }

        paginationControls.innerHTML = paginationHTML;

        // Setup pagination event listeners
        const paginationBtns = paginationControls.querySelectorAll('.pagination-btn');
        paginationBtns.forEach(btn => {
            btn.addEventListener('click', () => {
                const page = parseInt(btn.dataset.page);
                if (page && page !== this.state.currentPage) {
                    this.state.currentPage = page;
                    this.loadOrders();
                }
            });
        });
    }

    // Function untuk update order count display
    updateOrderCount() {
        const orderCount = document.getElementById('order-count');
        if (orderCount) {
            const startItem = (this.state.currentPage - 1) * this.state.itemsPerPage + 1;
            const endItem = Math.min(this.state.currentPage * this.state.itemsPerPage, this.state.totalOrders);
            
            orderCount.textContent = `Showing ${startItem}-${endItem} of ${this.state.totalOrders} orders`;
        }

        // Update pagination info
        const paginationInfo = document.getElementById('pagination-info');
        if (paginationInfo) {
            paginationInfo.textContent = `Showing ${this.state.orders.length} of ${this.state.totalOrders} orders`;
        }
    }

    // Function untuk update sort indicators
    updateSortIndicators() {
        const sortableHeaders = document.querySelectorAll('.sortable');

        sortableHeaders.forEach(header => {
            const icon = header.querySelector('i');
            if (!icon) return;

            if (header.dataset.sort === this.state.sortBy) {
                // Active sort column
                icon.className = this.state.sortOrder === 'asc' ? 'fas fa-sort-up' : 'fas fa-sort-down';
                header.classList.add('sort-active');
            } else {
                // Inactive sort column
                icon.className = 'fas fa-sort';
                header.classList.remove('sort-active');
            }
        });
    }

    // Function untuk clear all filters
    clearAllFilters() {
        // Reset filter state
        this.state.filters = {
            search: '',
            status: '',
            payment_method: ''
        };

        // Reset form inputs
        const searchInput = document.getElementById('order-search');
        const statusFilter = document.getElementById('status-filter');
        const paymentMethodFilter = document.getElementById('payment-method-filter');

        if (searchInput) searchInput.value = '';
        if (statusFilter) statusFilter.value = '';
        if (paymentMethodFilter) paymentMethodFilter.value = '';

        // Hide search clear button
        const searchClear = document.getElementById('search-clear');
        if (searchClear) searchClear.style.display = 'none';

        // Reset pagination and reload
        this.state.currentPage = 1;
        this.loadOrders();

        Utils.showNotification('Filters cleared', 'info');
    }

    // Function untuk export orders
    async exportOrders() {
        try {
            if (!confirm('Export orders data to CSV?')) {
                return;
            }

            Utils.showLoading('Preparing export...');

            // Get export data dari backend
            const response = await this.api.request(`${this.api.endpoints.payments}/admin/orders/export`, {
                method: 'POST',
                body: JSON.stringify({
                    format: 'csv',
                    filters: this.state.filters,
                    sort_by: this.state.sortBy,
                    sort_order: this.state.sortOrder
                })
            });

            if (!response.success) {
                throw new Error(response.message || 'Export failed');
            }

            // Create download link
            const blob = new Blob([response.data], { type: 'text/csv' });
            const url = window.URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `orders_export_${new Date().toISOString().split('T')[0]}.csv`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            window.URL.revokeObjectURL(url);

            Utils.hideLoading();
            Utils.showNotification('Orders exported successfully', 'success');

        } catch (error) {
            Utils.hideLoading();
            console.error('Export failed:', error);
            Utils.showNotification('Export failed: ' + error.message, 'error');
        }
    }

    // Function untuk show loading state
    showLoadingState() {
        const loadingOrders = document.getElementById('loading-orders');
        const emptyOrders = document.getElementById('empty-orders');
        
        if (loadingOrders) loadingOrders.style.display = 'block';
        if (emptyOrders) emptyOrders.style.display = 'none';
    }

    // Function untuk hide loading state
    hideLoadingState() {
        const loadingOrders = document.getElementById('loading-orders');
        if (loadingOrders) loadingOrders.style.display = 'none';
    }

    // Function untuk show empty state
    showEmptyState() {
        const emptyOrders = document.getElementById('empty-orders');
        const loadingOrders = document.getElementById('loading-orders');
        
        if (emptyOrders) emptyOrders.style.display = 'block';
        if (loadingOrders) loadingOrders.style.display = 'none';

        // Clear table body
        const tableBody = document.getElementById('orders-table-body');
        if (tableBody) tableBody.innerHTML = '';
    }

    // Function untuk setup real-time updates
    setupRealTimeUpdates() {
        // Setup real-time updates untuk order monitoring
        this.realTimeUpdates = this.api.setupRealTimeUpdates((eventType, data) => {
            switch (eventType) {
                case 'metrics_update':
                    this.updateRealTimeMetrics(data);
                    break;
                case 'order_updated':
                    this.handleOrderUpdate(data);
                    break;
                case 'new_order':
                    this.handleNewOrder(data);
                    break;
                default:
                    console.log('Real-time event:', eventType, data);
            }
        });
    }

    // Function untuk setup auto-refresh
    setupAutoRefresh() {
        // Auto-refresh every 30 seconds untuk order data
        this.refreshInterval = setInterval(async () => {
            if (!this.state.isLoading) {
                // Refresh real-time metrics tanpa full reload
                try {
                    const realTimeMetrics = await this.api.getRealTimeMetrics();
                    this.updateRealTimeMetrics(realTimeMetrics);
                    this.state.lastRefresh = new Date();
                } catch (error) {
                    console.warn('Auto-refresh failed:', error);
                }
            }
        }, 30000); // 30 seconds
    }

    // Function untuk handle real-time order updates
    handleOrderUpdate(data) {
        // Find dan update order in current list
        const orderIndex = this.state.orders.findIndex(order => order.id === data.order_id);
        if (orderIndex !== -1) {
            // Update order data
            this.state.orders[orderIndex] = { ...this.state.orders[orderIndex], ...data };
            
            // Re-render orders
            this.renderOrders();

            // Update modal jika terbuka untuk order ini
            if (this.orderModal.isOpen && this.orderModal.currentOrder.id === data.order_id) {
                this.orderModal.currentOrder = { ...this.orderModal.currentOrder, ...data };
                this.populateOrderModal(this.orderModal.currentOrder);
            }
        }
    }

    // Function untuk handle new orders
    handleNewOrder(data) {
        // Add new order ke beginning of list jika on first page
        if (this.state.currentPage === 1) {
            this.state.orders.unshift(data);
            
            // Remove last item jika exceeds page size
            if (this.state.orders.length > this.state.itemsPerPage) {
                this.state.orders.pop();
            }
            
            // Update total count
            this.state.totalOrders++;
            
            // Re-render orders
            this.renderOrders();
            this.updateOrderCount();

            // Show notification
            Utils.showNotification('New order received', 'info');
        }
    }

    // Function untuk update admin profile
    updateAdminProfile(adminUser) {
        const adminNameEl = document.getElementById('admin-name');
        if (adminNameEl) {
            adminNameEl.textContent = adminUser.full_name || 'Admin';
        }
    }

    // Function untuk handle logout
    async handleLogout() {
        try {
            if (!confirm('Are you sure you want to logout?')) {
                return;
            }

            // Cleanup resources
            this.cleanup();

            await this.auth.handleLogout();
        } catch (error) {
            console.error('Logout failed:', error);
            Utils.showNotification('Logout failed', 'error');
        }
    }

    // Function untuk cleanup resources
    cleanup() {
        // Destroy charts
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

        // Clear state
        this.state.orders = [];

        console.log('Order manager cleanup completed');
    }

    // Function untuk get current filter summary
    getFilterSummary() {
        const filters = [];

        if (this.state.filters.search) {
            filters.push(`Search: "${this.state.filters.search}"`);
        }

        if (this.state.filters.status) {
            filters.push(`Status: ${this.formatOrderStatus(this.state.filters.status)}`);
        }

        if (this.state.filters.payment_method) {
            filters.push(`Payment: ${this.formatPaymentMethod(this.state.filters.payment_method)}`);
        }

        return filters.length > 0 ? filters.join(', ') : 'No filters applied';
    }

    // Function untuk get order manager metrics untuk monitoring
    getOrderManagerMetrics() {
        return {
            total_orders_loaded: this.state.orders.length,
            current_page: this.state.currentPage,
            total_orders: this.state.totalOrders,
            active_filters: Object.values(this.state.filters).filter(f => f).length,
            charts_active: Object.values(this.charts).filter(chart => chart !== null).length,
            last_refresh: this.state.lastRefresh,
            auto_refresh_active: !!this.refreshInterval,
            real_time_updates_active: !!this.realTimeUpdates
        };
    }

    // Function untuk refresh all data
    async refreshAllData() {
        try {
            Utils.showLoading('Refreshing all order data...');

            // Reload semua data
            await this.loadInitialData();

            Utils.hideLoading();
            Utils.showNotification('Order data refreshed successfully', 'success');

        } catch (error) {
            Utils.hideLoading();
            console.error('Failed to refresh all data:', error);
            Utils.showNotification('Failed to refresh data: ' + error.message, 'error');
        }
    }
}

// Export OrderManager class untuk global access
window.OrderManager = OrderManager;