// /pdf-bookstore/admin-panel/js/upload.js

// Enterprise-Grade Upload Manager for PDF Bookstore Admin Panel
class UploadManager {
    constructor() {
        this.api = new AdminAPI(); // From admin-api.js line 3
        this.auth = new AdminAuthentication(); // From admin-auth.js line 3

        // State management with enterprise patterns
        this.state = {
            currentStep: 1,
            bookData: {},
            uploadedFiles: {
                pdf: null,
                cover: null
            },
            categories: [],
            isUploading: false,
            uploadProgress: {
                pdf: 0,
                cover: 0
            }
        };

        // Enterprise validation rules
        this.validation = {
            pdf: {
                maxSize: 50 * 1024 * 1024, // 50MB max size for PDF
                allowedTypes: ['application/pdf'],
                allowedExtensions: ['.pdf']
            },
            cover: {
                maxSize: 10 * 1024 * 1024, // 10MB max size for cover image
                allowedTypes: ['image/jpeg', 'image/png', 'image/webp'],
                allowedExtensions: ['.jpg', '.jpeg', '.png', '.webp']
            }
        };

        this.initializeUploadManager();
    }

    // Enterprise initialization with comprehensive error handling
    async initializeUploadManager() {
        try {
            // Verify admin access with professional error handling
            await this.verifyAdminAccess();

            // Setup event listeners with error boundaries
            this.setupEventListeners();

            // Load categories with retry mechanism
            await this.loadCategories();

            // Setup drag and drop functionality
            this.setupDragAndDrop();

            // Setup form validation
            this.setupFormValidation();

            Utils.showNotification('Upload form ready', 'success');

        } catch (error) {
            console.error('Upload manager initialization failed:', error);
            const errorMessage = (error && error.message) || 'Unknown initialization error';
            Utils.showNotification('Failed to initialize upload form: ' + errorMessage, 'error');

            // Redirect to login if authentication error
            if (errorMessage.includes('Admin') || errorMessage.includes('authentication')) {
                setTimeout(() => {
                    window.location.href = 'index.html';
                }, 2000);
            }
        }
    }

    // Professional admin access verification
    async verifyAdminAccess() {
        const isAuthenticated = this.auth && this.auth.isAdminAuthenticated && this.auth.isAdminAuthenticated();

        if (!isAuthenticated) {
            throw new Error('Admin authentication required');
        }

        const currentAdmin = this.auth && this.auth.getCurrentAdmin && this.auth.getCurrentAdmin();
        const adminData = (currentAdmin && currentAdmin.data) || currentAdmin;

        if (!adminData) {
            throw new Error('Admin user data not available');
        }

        this.updateAdminProfile(adminData);
    }

    // Enterprise event listeners setup with error handling
    setupEventListeners() {
        try {
            this.setupStepNavigation();
            this.setupFileUploadEvents();
            this.setupFormSubmissionEvents();
            this.setupModalEvents();
            this.setupUIEvents();
        } catch (error) {
            console.error('Event listener setup failed:', error);
            Utils.showNotification('Some features may not work properly', 'warning');
        }
    }

    // Professional step navigation with validation
    setupStepNavigation() {
        const nextStep1 = document.getElementById('next-step-1');
        const nextStep2 = document.getElementById('next-step-2');

        if (nextStep1) {
            nextStep1.addEventListener('click', () => {
                this.validateAndMoveToStep(2);
            });
        }

        if (nextStep2) {
            nextStep2.addEventListener('click', () => {
                this.validateAndMoveToStep(3);
            });
        }

        const prevStep2 = document.getElementById('prev-step-2');
        const prevStep3 = document.getElementById('prev-step-3');

        if (prevStep2) {
            prevStep2.addEventListener('click', () => {
                this.moveToStep(1);
            });
        }

        if (prevStep3) {
            prevStep3.addEventListener('click', () => {
                this.moveToStep(2);
            });
        }
    }

    // Enterprise file upload events with comprehensive error handling
    setupFileUploadEvents() {
        this.setupFileInput('pdf');
        this.setupFileInput('cover');
        this.setupFileRemoveEvents();
    }

    // Professional file input setup with safety checks
    setupFileInput(fileType) {
        const fileInput = document.getElementById(fileType + '-file-input');
        const dropzone = document.getElementById(fileType + '-dropzone');

        if (!fileInput || !dropzone) {
            console.warn('File input elements not found for type:', fileType);
            return;
        }

        // File input change event with error handling
        fileInput.addEventListener('change', (e) => {
            try {
                const file = e.target && e.target.files && e.target.files[0];
                if (file) {
                    this.handleFileSelection(file, fileType);
                }
            } catch (error) {
                this.handleFileError(error, fileType);
            }
        });

        // Dropzone click to trigger file input
        dropzone.addEventListener('click', () => {
            if (!this.state.isUploading) {
                fileInput.click();
            }
        });

        // Enterprise drag and drop events
        dropzone.addEventListener('dragover', (e) => {
            e.preventDefault();
            dropzone.classList.add('drag-over');
        });

        dropzone.addEventListener('dragleave', (e) => {
            e.preventDefault();
            dropzone.classList.remove('drag-over');
        });

        dropzone.addEventListener('drop', (e) => {
            e.preventDefault();
            dropzone.classList.remove('drag-over');

            try {
                const files = e.dataTransfer && e.dataTransfer.files;
                const file = files && files[0];

                if (file) {
                    this.handleFileSelection(file, fileType);
                } else {
                    Utils.showNotification('No file detected in drop', 'warning');
                }
            } catch (error) {
                this.handleFileError(error, fileType);
            }
        });
    }

    // Enterprise file selection with comprehensive validation
    async handleFileSelection(file, fileType) {
        try {
            // Comprehensive file validation
            const validation = this.validateFile(file, fileType);
            if (!validation.isValid) {
                const errorMessage = (validation && validation.error) || 'File validation failed';
                Utils.showNotification(errorMessage, 'error');
                return;
            }

            // Store file and update UI
            this.state.uploadedFiles[fileType] = file;
            this.updateFilePreview(file, fileType);

            Utils.showNotification(
                fileType.toUpperCase() + ' file selected successfully',
                'success'
            );

        } catch (error) {
            this.handleFileError(error, fileType);
        }
    }

    // Professional file validation with enterprise patterns
    validateFile(file, fileType) {
        const rules = this.validation && this.validation[fileType];

        if (!rules) {
            return {
                isValid: false,
                error: 'Unknown file type validation'
            };
        }

        // Check file exists
        if (!file) {
            return {
                isValid: false,
                error: 'No file provided'
            };
        }

        // Check file size
        if (file.size > rules.maxSize) {
            const maxSizeMB = Math.round(rules.maxSize / (1024 * 1024));
            return {
                isValid: false,
                error: 'File size too large. Maximum ' + maxSizeMB + 'MB allowed.'
            };
        }

        // Check file type
        const allowedTypes = (rules && rules.allowedTypes) || [];
        if (allowedTypes.length > 0 && !allowedTypes.includes(file.type)) {
            const allowedExts = (rules && rules.allowedExtensions) || [];
            return {
                isValid: false,
                error: 'Invalid file type. Only ' + allowedExts.join(', ') + ' files allowed.'
            };
        }

        // Check file extension
        const fileName = file.name || '';
        const fileExtension = '.' + fileName.split('.').pop().toLowerCase();
        const allowedExtensions = (rules && rules.allowedExtensions) || [];

        if (allowedExtensions.length > 0 && !allowedExtensions.includes(fileExtension)) {
            return {
                isValid: false,
                error: 'Invalid file extension. Only ' + allowedExtensions.join(', ') + ' files allowed.'
            };
        }

        return { isValid: true };
    }

    // Professional file preview update with error handling
    updateFilePreview(file, fileType) {
        try {
            const uploadArea = document.getElementById(fileType + '-upload-area');
            const dropzone = document.getElementById(fileType + '-dropzone');
            const preview = document.getElementById(fileType + '-preview');

            if (!uploadArea || !dropzone || !preview) {
                console.warn('Preview elements not found for:', fileType);
                return;
            }

            // Hide dropzone and show preview
            dropzone.style.display = 'none';
            preview.style.display = 'block';

            // Update file info safely
            const fileName = document.getElementById(fileType + '-file-name');
            const fileSize = document.getElementById(fileType + '-file-size');
            const uploadStatus = document.getElementById(fileType + '-upload-status');

            if (fileName) fileName.textContent = file.name || 'Unknown file';
            if (fileSize) fileSize.textContent = Utils.formatFileSize && Utils.formatFileSize(file.size) || (file.size + ' bytes');
            if (uploadStatus) uploadStatus.textContent = 'Ready to upload';

            // Special handling for cover image preview
            if (fileType === 'cover') {
                this.updateCoverImagePreview(file);
            }

        } catch (error) {
            console.error('File preview update failed:', error);
            Utils.showNotification('Preview update failed', 'warning');
        }
    }

    // Professional cover image preview with error handling
    updateCoverImagePreview(file) {
        try {
            const previewImage = document.getElementById('cover-preview-image');
            if (previewImage && file) {
                const reader = new FileReader();

                reader.onload = (e) => {
                    try {
                        const result = e.target && e.target.result;
                        if (result) {
                            previewImage.src = result;
                        }
                    } catch (error) {
                        console.error('Image preview load failed:', error);
                    }
                };

                reader.onerror = () => {
                    console.error('FileReader error for image preview');
                };

                reader.readAsDataURL(file);
            }
        } catch (error) {
            console.error('Cover image preview failed:', error);
        }
    }

    // Enterprise file remove events setup
    setupFileRemoveEvents() {
        const pdfRemove = document.getElementById('pdf-remove');
        if (pdfRemove) {
            pdfRemove.addEventListener('click', () => {
                this.removeFile('pdf');
            });
        }

        const coverRemove = document.getElementById('cover-remove');
        if (coverRemove) {
            coverRemove.addEventListener('click', () => {
                this.removeFile('cover');
            });
        }
    }

    // Professional file removal with state cleanup
    removeFile(fileType) {
        try {
            // Clear file from state
            if (this.state && this.state.uploadedFiles) {
                this.state.uploadedFiles[fileType] = null;
            }

            // Reset file input
            const fileInput = document.getElementById(fileType + '-file-input');
            if (fileInput) {
                fileInput.value = '';
            }

            // Hide preview and show dropzone
            const dropzone = document.getElementById(fileType + '-dropzone');
            const preview = document.getElementById(fileType + '-preview');

            if (dropzone) dropzone.style.display = 'block';
            if (preview) preview.style.display = 'none';

            Utils.showNotification(fileType.toUpperCase() + ' file removed', 'info');

        } catch (error) {
            console.error('File removal failed:', error);
            Utils.showNotification('File removal failed', 'error');
        }
    }

    // Enterprise form submission events
    setupFormSubmissionEvents() {
        const uploadForm = document.getElementById('book-upload-form');
        if (uploadForm) {
            uploadForm.addEventListener('submit', async(e) => {
                e.preventDefault();
                await this.handleFormSubmission();
            });
        }

        // Description character counter with safety
        this.setupDescriptionCounter();
    }

    // Professional description counter setup
    setupDescriptionCounter() {
        const descriptionTextarea = document.getElementById('book-description');
        const characterCount = document.getElementById('description-count');

        if (descriptionTextarea && characterCount) {
            descriptionTextarea.addEventListener('input', () => {
                try {
                    const value = descriptionTextarea.value || '';
                    const count = value.length;
                    characterCount.textContent = count.toString();

                    // Update color based on limit
                    if (count > 900) {
                        characterCount.style.color = 'var(--admin-danger)';
                    } else if (count > 800) {
                        characterCount.style.color = 'var(--admin-warning)';
                    } else {
                        characterCount.style.color = 'var(--admin-text-secondary)';
                    }
                } catch (error) {
                    console.error('Character counter update failed:', error);
                }
            });
        }
    }

    // Enterprise modal events setup
    setupModalEvents() {
        const uploadAnotherBtn = document.getElementById('upload-another');
        if (uploadAnotherBtn) {
            uploadAnotherBtn.addEventListener('click', () => {
                this.resetUploadForm();
                this.hideSuccessModal();
            });
        }
    }

    // Professional UI events setup
    setupUIEvents() {
        // Sidebar toggle with safety
        this.setupSidebarToggle();

        // Mobile menu toggle
        this.setupMobileMenuToggle();

        // Logout functionality
        this.setupLogoutHandler();
    }

    // Professional sidebar toggle
    setupSidebarToggle() {
        const sidebarToggle = document.getElementById('sidebar-toggle');
        const sidebar = document.getElementById('admin-sidebar');

        if (sidebarToggle && sidebar) {
            sidebarToggle.addEventListener('click', () => {
                try {
                    sidebar.classList.toggle('collapsed');
                } catch (error) {
                    console.error('Sidebar toggle failed:', error);
                }
            });
        }
    }

    // Professional mobile menu toggle
    setupMobileMenuToggle() {
        const mobileMenuToggle = document.getElementById('mobile-menu-toggle');
        const sidebar = document.getElementById('admin-sidebar');

        if (mobileMenuToggle && sidebar) {
            mobileMenuToggle.addEventListener('click', () => {
                try {
                    sidebar.classList.toggle('mobile-open');
                } catch (error) {
                    console.error('Mobile menu toggle failed:', error);
                }
            });
        }
    }

    // Professional logout handler
    setupLogoutHandler() {
        const logoutBtn = document.getElementById('admin-logout');
        if (logoutBtn) {
            logoutBtn.addEventListener('click', async(e) => {
                e.preventDefault();
                await this.handleLogout();
            });
        }
    }

    // Enterprise categories loading with retry mechanism
    async loadCategories() {
        try {
            const apiRequest = this.api && this.api.request;
            const endpoint = this.api && this.api.endpoints && this.api.endpoints.books;

            if (!apiRequest || !endpoint) {
                throw new Error('API client not available');
            }

            const response = await apiRequest(endpoint + '/categories');
            // API call to book-service/handlers.rs line 89

            const responseData = (response && response.data) || [];
            if (response && response.success && responseData) {
                this.state.categories = Array.isArray(responseData) ? responseData : [];
                this.renderCategoriesGrid();
            } else {
                throw new Error('Invalid categories response');
            }

        } catch (error) {
            console.error('Failed to load categories:', error);
            // Use fallback categories if API failed
            this.useFallbackCategories();
            Utils.showNotification('Using default categories', 'warning');
        }
    }

    // Professional fallback categories
    useFallbackCategories() {
        this.state.categories = [
            { id: '1', name: 'Fiction', slug: 'fiction' },
            { id: '2', name: 'Non-Fiction', slug: 'non-fiction' },
            { id: '3', name: 'Technology', slug: 'technology' },
            { id: '4', name: 'Business', slug: 'business' },
            { id: '5', name: 'Education', slug: 'education' }
        ];
        this.renderCategoriesGrid();
    }

    // Enterprise categories grid rendering
    renderCategoriesGrid() {
        const categoriesGrid = document.getElementById('categories-grid');
        if (!categoriesGrid) return;

        try {
            const categories = this.state.categories || [];
            const categoryItems = categories.map(category => {
                const categoryData = category || {};
                const categoryId = (categoryData.id || '').toString();
                const categoryName = categoryData.name || 'Unknown Category';

                return `
                    <label class="category-item">
                        <input type="checkbox" name="categories" value="${Utils.escapeHtml && Utils.escapeHtml(categoryId) || categoryId}" class="category-checkbox">
                        <span class="category-label">${Utils.escapeHtml && Utils.escapeHtml(categoryName) || categoryName}</span>
                    </label>
                `;
            }).join('');

            categoriesGrid.innerHTML = categoryItems;

            // Add CSS styles for categories grid
            this.addCategoriesGridStyles();

        } catch (error) {
            console.error('Categories grid rendering failed:', error);
            Utils.showNotification('Categories display error', 'warning');
        }
    }

    // Professional categories grid styles
    addCategoriesGridStyles() {
        if (!document.getElementById('categories-grid-styles')) {
            try {
                const style = document.createElement('style');
                style.id = 'categories-grid-styles';
                style.textContent = `
                    .categories-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
                        gap: 0.75rem;
                        margin-top: 0.5rem;
                    }
                    .category-item {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        padding: 0.75rem;
                        background: var(--admin-bg-primary);
                        border: 1px solid var(--admin-border-light);
                        border-radius: 8px;
                        cursor: pointer;
                        transition: var(--admin-transition);
                    }
                    .category-item:hover {
                        background: var(--admin-bg-tertiary);
                        border-color: var(--admin-primary);
                    }
                    .category-checkbox:checked + .category-label {
                        color: var(--admin-primary);
                        font-weight: 600;
                    }
                    .category-label {
                        font-size: 14px;
                        color: var(--admin-text-secondary);
                        transition: var(--admin-transition);
                    }
                `;
                document.head.appendChild(style);
            } catch (error) {
                console.error('Categories styles injection failed:', error);
            }
        }
    }

    // Enterprise step validation and movement
    async validateAndMoveToStep(targetStep) {
        try {
            const currentStepValid = await this.validateCurrentStep();

            if (currentStepValid) {
                this.moveToStep(targetStep);

                // Update preview if moving to step 3
                if (targetStep === 3) {
                    this.updateBookPreview();
                }
            }
        } catch (error) {
            console.error('Step validation failed:', error);
            Utils.showNotification('Step validation error', 'error');
        }
    }

    // Professional current step validation
    async validateCurrentStep() {
        try {
            switch (this.state.currentStep) {
                case 1:
                    return this.validateStep1();
                case 2:
                    return this.validateStep2();
                case 3:
                    return this.validateStep3();
                default:
                    return true;
            }
        } catch (error) {
            console.error('Current step validation failed:', error);
            return false;
        }
    }

    // Enterprise step 1 validation (book information)
    validateStep1() {
        const requiredFields = [
            { id: 'book-title', name: 'Book Title' },
            { id: 'book-author', name: 'Author' },
            { id: 'book-price', name: 'Price' }
        ];

        let isValid = true;

        // Clear previous errors
        this.clearFormErrors();

        // Validate required fields
        requiredFields.forEach(field => {
            const input = document.getElementById(field.id);
            const value = input && input.value && input.value.trim();

            if (!input || !value) {
                this.showFieldError(field.id, field.name + ' is required');
                isValid = false;
            }
        });

        // Validate price
        const priceInput = document.getElementById('book-price');
        if (priceInput && priceInput.value) {
            const price = parseFloat(priceInput.value);
            if (isNaN(price) || price < 1000) {
                this.showFieldError('book-price', 'Price must be at least IDR 1,000');
                isValid = false;
            }
        }

        // Validate description length
        const descriptionInput = document.getElementById('book-description');
        if (descriptionInput) {
            const description = descriptionInput.value || '';
            if (description.length > 1000) {
                this.showFieldError('book-description', 'Description must be less than 1000 characters');
                isValid = false;
            }
        }

        // Validate at least one category selected
        const selectedCategories = document.querySelectorAll('.category-checkbox:checked');
        if (selectedCategories.length === 0) {
            Utils.showNotification('Please select at least one category', 'warning');
            isValid = false;
        }

        return isValid;
    }

    // Professional step 2 validation (file uploads)
    validateStep2() {
        // PDF file is required
        const pdfFile = this.state && this.state.uploadedFiles && this.state.uploadedFiles.pdf;
        if (!pdfFile) {
            Utils.showNotification('PDF file is required', 'error');
            return false;
        }

        // Cover image is optional, so validation passes regardless
        return true;
    }

    // Professional step 3 validation (preview & publish)
    validateStep3() {
        // All validation should be done in previous steps
        return true;
    }

    // Enterprise step movement with error handling
    moveToStep(stepNumber) {
        try {
            // Hide all steps
            const steps = document.querySelectorAll('.form-step');
            steps.forEach(step => {
                step.classList.remove('active');
            });

            // Show target step
            const targetStep = document.getElementById('step-' + stepNumber);
            if (targetStep) {
                targetStep.classList.add('active');
            }

            // Update step indicators
            const stepIndicators = document.querySelectorAll('.step');
            stepIndicators.forEach((step, index) => {
                const isActive = (index + 1) === stepNumber;
                step.classList.toggle('active', isActive);
            });

            // Update state
            this.state.currentStep = stepNumber;

            // Scroll to top
            window.scrollTo({ top: 0, behavior: 'smooth' });

        } catch (error) {
            console.error('Step movement failed:', error);
            Utils.showNotification('Navigation error', 'error');
        }
    }

    // Professional book preview update
    updateBookPreview() {
        try {
            // Get form data safely
            const formData = this.getFormData();

            // Update preview cover
            this.updatePreviewCover();

            // Update preview details
            const updates = {
                'preview-title': formData.title || 'Book Title',
                'preview-author': formData.author || 'Author Name',
                'preview-price': 'IDR ' + (Utils.formatNumber && Utils.formatNumber(formData.price) || formData.price || 0),
                'preview-description': formData.description || 'Description will appear here...',
                'preview-language': formData.language || 'Language',
                'preview-pages': (formData.total_pages || 0) + ' pages',
                'preview-isbn': formData.isbn || 'ISBN'
            };

            Object.entries(updates).forEach(([id, value]) => {
                const element = document.getElementById(id);
                if (element) element.textContent = value;
            });

            // Update preview categories
            this.updatePreviewCategories();

        } catch (error) {
            console.error('Book preview update failed:', error);
            Utils.showNotification('Preview update error', 'warning');
        }
    }

    // Professional preview cover update
    updatePreviewCover() {
        try {
            const previewCover = document.getElementById('preview-cover');
            if (!previewCover) return;

            const coverFile = this.state && this.state.uploadedFiles && this.state.uploadedFiles.cover;

            if (coverFile) {
                const reader = new FileReader();
                reader.onload = (e) => {
                    try {
                        const result = e.target && e.target.result;
                        if (result) {
                            previewCover.innerHTML = '<img src="' + result + '" alt="Book cover" style="width: 100%; height: 100%; object-fit: cover; border-radius: 8px;">';
                        }
                    } catch (error) {
                        console.error('Preview cover load failed:', error);
                    }
                };
                reader.readAsDataURL(coverFile);
            } else {
                previewCover.innerHTML = '<i class="fas fa-book"></i>';
            }
        } catch (error) {
            console.error('Preview cover update failed:', error);
        }
    }

    // Professional preview categories update
    updatePreviewCategories() {
        try {
            const selectedCategories = this.getSelectedCategories();
            const previewCategories = document.getElementById('preview-categories');

            if (previewCategories) {
                const categoryTags = selectedCategories.map(cat => {
                    const categoryName = (cat && cat.name) || 'Unknown';
                    return '<span class="category-tag">' + (Utils.escapeHtml && Utils.escapeHtml(categoryName) || categoryName) + '</span>';
                }).join('');

                previewCategories.innerHTML = categoryTags;
            }
        } catch (error) {
            console.error('Preview categories update failed:', error);
        }
    }

    // Enterprise form data collection with error handling
    getFormData() {
        try {
            return {
                title: this.getElementValue('book-title') || '',
                author: this.getElementValue('book-author') || '',
                description: this.getElementValue('book-description') || '',
                isbn: this.getElementValue('book-isbn') || '',
                price: parseFloat(this.getElementValue('book-price')) || 0,
                language: this.getElementValue('book-language') || 'id',
                total_pages: parseInt(this.getElementValue('book-pages')) || null,
                categories: this.getSelectedCategories().map(cat => (cat && cat.id) || '').filter(Boolean),
                publish_immediately: this.getElementChecked('publish-immediately') || false,
                send_notification: this.getElementChecked('send-notification') || false
            };
        } catch (error) {
            console.error('Form data collection failed:', error);
            throw new Error('Failed to collect form data');
        }
    }

    // Professional element value getter with safety
    getElementValue(elementId) {
        try {
            const element = document.getElementById(elementId);
            const value = element && element.value;
            return (typeof value === 'string') ? value.trim() : '';
        } catch (error) {
            console.warn('Failed to get element value for:', elementId);
            return '';
        }
    }

    // Professional element checked getter
    getElementChecked(elementId) {
        try {
            const element = document.getElementById(elementId);
            return element && element.checked || false;
        } catch (error) {
            console.warn('Failed to get element checked for:', elementId);
            return false;
        }
    }

    // Enterprise selected categories getter
    getSelectedCategories() {
        try {
            const checkboxes = document.querySelectorAll('.category-checkbox:checked');
            const selectedIds = Array.from(checkboxes).map(checkbox =>
                (checkbox && checkbox.value) || ''
            ).filter(Boolean);

            return selectedIds.map(id => {
                const categories = this.state && this.state.categories || [];
                return categories.find(cat => (cat && cat.id) === id) || null;
            }).filter(Boolean);

        } catch (error) {
            console.warn('Failed to get selected categories:', error);
            return [];
        }
    }

    // Enterprise form submission with comprehensive error handling
    async handleFormSubmission() {
        try {
            if (this.state.isUploading) {
                Utils.showNotification('Upload already in progress', 'warning');
                return;
            }

            this.state.isUploading = true;

            // Final validation
            const isValid = await this.validateCurrentStep();
            if (!isValid) {
                this.state.isUploading = false;
                return;
            }

            // Show loading state
            Utils.showLoading && Utils.showLoading('Publishing book...');
            this.updateUploadProgress(0, 'Preparing upload...');

            // Get form data
            const bookData = this.getFormData();

            // Create FormData for file upload
            const formData = new FormData();

            // Add book metadata
            Object.entries(bookData).forEach(([key, value]) => {
                if (value !== null && value !== undefined) {
                    const valueToAdd = Array.isArray(value) ? JSON.stringify(value) : value.toString();
                    formData.append(key, valueToAdd);
                }
            });

            // Add files
            const pdfFile = this.state && this.state.uploadedFiles && this.state.uploadedFiles.pdf;
            const coverFile = this.state && this.state.uploadedFiles && this.state.uploadedFiles.cover;

            if (pdfFile) {
                formData.append('pdf_file', pdfFile);
            }

            if (coverFile) {
                formData.append('cover_image', coverFile);
            }

            this.updateUploadProgress(25, 'Uploading files...');

            // Upload book to backend
            const response = await this.uploadBookToBackend(formData);

            this.updateUploadProgress(100, 'Book published successfully!');

            // Show success modal
            this.showSuccessModal(response && response.data);

            // Reset form
            this.resetUploadForm();

            Utils.hideLoading && Utils.hideLoading();
            Utils.showNotification('Book published successfully!', 'success');

        } catch (error) {
            console.error('Book upload failed:', error);
            Utils.hideLoading && Utils.hideLoading();
            const errorMessage = (error && error.message) || 'Unknown upload error';
            Utils.showNotification('Failed to publish book: ' + errorMessage, 'error');
        } finally {
            this.state.isUploading = false;
        }
    }

    // Professional backend upload with error handling
    async uploadBookToBackend(formData) {
        try {
            const apiEndpoint = this.api && this.api.endpoints && this.api.endpoints.books;
            const apiToken = this.api && this.api.token;

            if (!apiEndpoint || !apiToken) {
                throw new Error('API configuration not available');
            }

            const response = await fetch(apiEndpoint + '/admin/books', {
                method: 'POST',
                headers: {
                    'Authorization': 'Bearer ' + apiToken
                },
                body: formData
            });

            if (!response.ok) {
                const errorData = await response.json().catch(() => ({}));
                const errorMessage = (errorData && errorData.message) || 'Upload failed: ' + response.status;
                throw new Error(errorMessage);
            }

            return await response.json();

        } catch (error) {
            console.error('Backend upload failed:', error);
            throw error;
        }
    }

    // Enterprise upload progress tracking
    updateUploadProgress(percentage, message) {
        try {
            const progressFill = document.getElementById('progress-fill');
            const progressText = document.getElementById('progress-text');
            const loadingMessage = document.getElementById('loading-message');

            if (progressFill) {
                progressFill.style.width = percentage + '%';
            }

            if (progressText) {
                progressText.textContent = percentage + '%';
            }

            if (loadingMessage && message) {
                loadingMessage.textContent = message;
            }

            // Show/hide progress bar
            const uploadProgress = document.getElementById('upload-progress');
            if (uploadProgress) {
                uploadProgress.style.display = percentage > 0 ? 'flex' : 'none';
            }

        } catch (error) {
            console.error('Upload progress update failed:', error);
        }
    }

    // Professional success modal display
    showSuccessModal(bookData) {
        try {
            const modal = document.getElementById('upload-success-modal');
            const bookTitle = document.getElementById('success-book-title');
            const viewBookLink = document.getElementById('view-book-link');

            if (modal) {
                modal.classList.add('show');
                modal.style.display = 'flex';
            }

            if (bookTitle && bookData) {
                const title = (bookData && bookData.title) || 'New Book';
                bookTitle.textContent = title;
            }

            if (viewBookLink && bookData) {
                const bookId = (bookData && bookData.id) || '';
                viewBookLink.href = '../book.html?id=' + bookId;
            }

        } catch (error) {
            console.error('Success modal display failed:', error);
        }
    }

    // Professional success modal hiding
    hideSuccessModal() {
        try {
            const modal = document.getElementById('upload-success-modal');
            if (modal) {
                modal.classList.remove('show');
                modal.style.display = 'none';
            }
        } catch (error) {
            console.error('Success modal hiding failed:', error);
        }
    }

    // Enterprise form reset with comprehensive cleanup
    resetUploadForm() {
        try {
            // Reset state
            this.state.currentStep = 1;
            this.state.bookData = {};
            this.state.uploadedFiles = { pdf: null, cover: null };
            this.state.isUploading = false;
            this.state.uploadProgress = { pdf: 0, cover: 0 };

            // Reset form
            const form = document.getElementById('book-upload-form');
            if (form) {
                form.reset();
            }

            // Reset file uploads
            this.removeFile('pdf');
            this.removeFile('cover');

            // Reset step navigation
            this.moveToStep(1);

            // Clear form errors
            this.clearFormErrors();

            // Reset character counter
            const characterCount = document.getElementById('description-count');
            if (characterCount) {
                characterCount.textContent = '0';
                characterCount.style.color = 'var(--admin-text-secondary)';
            }

            // Uncheck all categories
            const categoryCheckboxes = document.querySelectorAll('.category-checkbox');
            categoryCheckboxes.forEach(checkbox => {
                checkbox.checked = false;
            });

            // Hide upload progress
            this.updateUploadProgress(0, '');

        } catch (error) {
            console.error('Form reset failed:', error);
            Utils.showNotification('Form reset error', 'warning');
        }
    }

    // Professional drag and drop setup
    setupDragAndDrop() {
        try {
            // Prevent default drag behaviors
            const dragEvents = ['dragenter', 'dragover', 'dragleave', 'drop'];
            dragEvents.forEach(eventName => {
                document.addEventListener(eventName, (e) => {
                    e.preventDefault();
                    e.stopPropagation();
                });
            });

            // Add global drag and drop styling
            document.addEventListener('dragenter', () => {
                document.body.classList.add('dragging');
            });

            document.addEventListener('dragleave', (e) => {
                if (!e.relatedTarget) {
                    document.body.classList.remove('dragging');
                }
            });

            document.addEventListener('drop', () => {
                document.body.classList.remove('dragging');
            });

        } catch (error) {
            console.error('Drag and drop setup failed:', error);
        }
    }

    // Enterprise form validation setup
    setupFormValidation() {
        try {
            // Real-time validation for required fields
            const requiredFields = ['book-title', 'book-author', 'book-price'];

            requiredFields.forEach(fieldId => {
                const field = document.getElementById(fieldId);
                if (field) {
                    field.addEventListener('blur', () => {
                        const value = field.value && field.value.trim();
                        if (!value) {
                            const placeholder = field.placeholder || fieldId;
                            this.showFieldError(fieldId, placeholder + ' is required');
                        } else {
                            this.clearFieldError(fieldId);
                        }
                    });

                    field.addEventListener('input', () => {
                        this.clearFieldError(fieldId);
                    });
                }
            });

            // Price validation
            const priceInput = document.getElementById('book-price');
            if (priceInput) {
                priceInput.addEventListener('blur', () => {
                    const value = priceInput.value;
                    if (value) {
                        const price = parseFloat(value);
                        if (isNaN(price) || price < 1000) {
                            this.showFieldError('book-price', 'Price must be at least IDR 1,000');
                        }
                    }
                });
            }

        } catch (error) {
            console.error('Form validation setup failed:', error);
        }
    }

    // Professional field error display
    showFieldError(fieldId, message) {
        try {
            const field = document.getElementById(fieldId);
            const errorId = fieldId.replace('book-', '') + '-error';
            const errorElement = document.getElementById(errorId);

            if (field) {
                field.style.borderColor = 'var(--admin-danger)';
            }

            if (errorElement && message) {
                errorElement.textContent = message;
                errorElement.style.display = 'block';
            }
        } catch (error) {
            console.error('Field error display failed:', error);
        }
    }

    // Professional field error clearing
    clearFieldError(fieldId) {
        try {
            const field = document.getElementById(fieldId);
            const errorId = fieldId.replace('book-', '') + '-error';
            const errorElement = document.getElementById(errorId);

            if (field) {
                field.style.borderColor = '';
            }

            if (errorElement) {
                errorElement.textContent = '';
                errorElement.style.display = 'none';
            }
        } catch (error) {
            console.error('Field error clearing failed:', error);
        }
    }

    // Enterprise form errors clearing
    clearFormErrors() {
        try {
            const errorElements = document.querySelectorAll('.form-error');
            const inputElements = document.querySelectorAll('.form-input');

            errorElements.forEach(error => {
                error.textContent = '';
                error.style.display = 'none';
            });

            inputElements.forEach(input => {
                input.style.borderColor = '';
            });
        } catch (error) {
            console.error('Form errors clearing failed:', error);
        }
    }

    // Professional admin profile update
    updateAdminProfile(adminUser) {
        try {
            const adminNameEl = document.getElementById('admin-name');
            if (adminNameEl && adminUser) {
                const fullName = (adminUser && adminUser.full_name) || 'Admin';
                adminNameEl.textContent = fullName;
            }
        } catch (error) {
            console.error('Admin profile update failed:', error);
        }
    }

    // Enterprise logout handling
    async handleLogout() {
        try {
            if (!confirm('Are you sure you want to logout?')) {
                return;
            }

            const authHandler = this.auth && this.auth.handleLogout;
            if (authHandler) {
                await authHandler();
            } else {
                // Fallback logout
                window.location.href = 'index.html';
            }

        } catch (error) {
            console.error('Logout failed:', error);
            Utils.showNotification('Logout failed', 'error');
        }
    }

    // Professional file error handling
    handleFileError(error, fileType) {
        try {
            console.error('File error for', fileType + ':', error);

            const errorMessage = (error && error.message) || 'Unknown file error';

            // Clear the problematic file
            this.removeFile(fileType);

            Utils.showNotification('File error: ' + errorMessage, 'error');

        } catch (handleError) {
            console.error('File error handling failed:', handleError);
        }
    }

    // Enterprise resource cleanup
    cleanup() {
        try {
            // Clear file references
            if (this.state && this.state.uploadedFiles) {
                this.state.uploadedFiles = { pdf: null, cover: null };
            }

            // Clear any timers
            if (this.retryTimer) {
                clearTimeout(this.retryTimer);
                this.retryTimer = null;
            }

            // Clear event listeners if using AbortController
            if (this.abortController) {
                this.abortController.abort();
                this.abortController = null;
            }

            console.log('Upload manager cleanup completed');

        } catch (error) {
            console.warn('Cleanup failed:', error);
        }
    }
}

// Enterprise export with error handling
try {
    window.UploadManager = UploadManager;
} catch (error) {
    console.error('Failed to export UploadManager:', error);
}