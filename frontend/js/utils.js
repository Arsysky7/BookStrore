// /pdf-bookstore/frontend/js/utils.js

class Utils {
    static showLoading(message = 'Loading...', containerId = 'loading-container') {
        let loadingElement = document.getElementById(containerId);

        if (!loadingElement) {
            loadingElement = document.createElement('div');
            loadingElement.id = containerId;
            loadingElement.className = 'loading-overlay';
            document.body.appendChild(loadingElement);
        }

        loadingElement.innerHTML = `
            <div class="loading-content">
                <div class="spinner"></div>
                <div class="loading-message">${this.escapeHtml(message)}</div>
            </div>
        `;

        loadingElement.style.display = 'flex';
        loadingElement.setAttribute('aria-live', 'polite');
    }

    static hideLoading(containerId = 'loading-container') {
        const loadingElement = document.getElementById(containerId);

        if (loadingElement) {
            loadingElement.style.display = 'none';
            loadingElement.removeAttribute('aria-live');
        }
    }

    static showNotification(message, type = 'info', duration = 5000) {
        let notificationContainer = document.getElementById('notification-container');

        if (!notificationContainer) {
            notificationContainer = document.createElement('div');
            notificationContainer.id = 'notification-container';
            notificationContainer.className = 'notification-container';
            document.body.appendChild(notificationContainer);
        }

        const notification = document.createElement('div');
        notification.className = `notification notification-${type}`;
        notification.setAttribute('role', 'alert');

        notification.innerHTML = `
            <div class="notification-content">
                <span class="notification-message">${this.escapeHtml(message)}</span>
                <button class="notification-close" aria-label="Close notification">&times;</button>
            </div>
        `;

        notificationContainer.appendChild(notification);

        const closeButton = notification.querySelector('.notification-close');
        closeButton.addEventListener('click', () => {
            this.removeNotification(notification);
        });

        if (duration > 0) {
            setTimeout(() => {
                this.removeNotification(notification);
            }, duration);
        }

        if (window.requestAnimationFrame) {
            requestAnimationFrame(() => {
                notification.classList.add('notification-show');
            });
        } else {
            setTimeout(() => {
                notification.classList.add('notification-show');
            }, 10);
        }

        return notification;
    }

    static removeNotification(notification) {
        notification.classList.add('notification-hide');

        setTimeout(() => {
            if (notification.parentNode) {
                notification.parentNode.removeChild(notification);
            }
        }, 300);
    }

    static debounce(func, wait, immediate = false) {
        let timeout;

        return function executedFunction(...args) {
            const later = () => {
                timeout = null;
                if (!immediate) func(...args);
            };

            const callNow = immediate && !timeout;
            clearTimeout(timeout);
            timeout = setTimeout(later, wait);

            if (callNow) func(...args);
        };
    }

    static throttle(func, limit) {
        let inThrottle;
        return function() {
            const args = arguments;
            const context = this;

            if (!inThrottle) {
                func.apply(context, args);
                inThrottle = true;
                setTimeout(() => inThrottle = false, limit);
            }
        };
    }

    static isValidEmail(email) {
        const emailRegex = /^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/;
        return emailRegex.test(email);
    }

    static validatePassword(password) {
        const result = {
            isValid: false,
            errors: [],
            strength: 'weak'
        };

        if (password.length < 6) {
            result.errors.push('Password must be at least 6 characters long');
        }

        if (password.length > 128) {
            result.errors.push('Password must be less than 128 characters');
        }

        const weakPatterns = [
            /^123456/,
            /password/i,
            /^qwerty/i,
        ];

        const hasWeakPattern = weakPatterns.some(pattern => pattern.test(password));

        if (hasWeakPattern) {
            result.errors.push('Password contains common weak patterns');
        }

        let strengthScore = 0;

        if (password.length >= 8) strengthScore++;
        if (/[a-z]/.test(password)) strengthScore++;
        if (/[A-Z]/.test(password)) strengthScore++;
        if (/[0-9]/.test(password)) strengthScore++;
        if (/[^a-zA-Z0-9]/.test(password)) strengthScore++;

        if (strengthScore >= 4) {
            result.strength = 'strong';
        } else if (strengthScore >= 2) {
            result.strength = 'medium';
        } else {
            result.strength = 'weak';
        }

        result.isValid = result.errors.length === 0 && strengthScore >= 2;

        return result;
    }

    static escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    static truncateText(text, maxLength, suffix = '...') {
        if (!text || typeof text !== 'string') {
            return '';
        }

        if (text.length <= maxLength) {
            return text;
        }

        return text.substring(0, maxLength - suffix.length) + suffix;
    }

    static formatFileSize(bytes) {
        if (typeof bytes !== 'number' || bytes < 0) {
            return '0 B';
        }

        const units = ['B', 'KB', 'MB', 'GB', 'TB'];

        let size = bytes;
        let unitIndex = 0;

        while (size >= 1024 && unitIndex < units.length - 1) {
            size /= 1024;
            unitIndex++;
        }

        const decimals = unitIndex === 0 ? 0 : 1;
        return `${size.toFixed(decimals)} ${units[unitIndex]}`;
    }

    static formatCurrency(amount) {
        return new Intl.NumberFormat('id-ID', {
            style: 'currency',
            currency: 'IDR',
            minimumFractionDigits: 0,
            maximumFractionDigits: 0
        }).format(amount);
    }

    static formatDate(date, options = {}) {
        const defaultOptions = {
            year: 'numeric',
            month: 'long',
            day: 'numeric',
            ...options
        };

        return new Intl.DateTimeFormat('id-ID', defaultOptions).format(new Date(date));
    }

    static formatRelativeTime(date) {
        const now = new Date();
        const diffInSeconds = Math.floor((now - new Date(date)) / 1000);

        const units = [
            { name: 'year', seconds: 31536000 },
            { name: 'month', seconds: 2592000 },
            { name: 'week', seconds: 604800 },
            { name: 'day', seconds: 86400 },
            { name: 'hour', seconds: 3600 },
            { name: 'minute', seconds: 60 }
        ];

        for (const unit of units) {
            const count = Math.floor(diffInSeconds / unit.seconds);

            if (count >= 1) {
                return `${count} ${unit.name}${count !== 1 ? 's' : ''} ago`;
            }
        }

        return 'Just now';
    }

    static async copyToClipboard(text) {
        try {
            if (navigator.clipboard && window.isSecureContext) {
                await navigator.clipboard.writeText(text);
                return true;
            } else {
                const textArea = document.createElement('textarea');
                textArea.value = text;
                textArea.style.position = 'fixed';
                textArea.style.opacity = '0';
                document.body.appendChild(textArea);

                textArea.select();
                document.execCommand('copy');
                document.body.removeChild(textArea);

                return true;
            }
        } catch (error) {
            console.error('Failed to copy to clipboard:', error);
            return false;
        }
    }

    static generateId(prefix = 'id') {
        const timestamp = Date.now().toString(36);
        const random = Math.random().toString(36).substr(2, 5);
        return `${prefix}-${timestamp}-${random}`;
    }

    static scrollToElement(element, offset = 0, behavior = 'smooth') {
        const targetElement = typeof element === 'string' ?
            document.querySelector(element) : element;

        if (!targetElement) {
            console.warn('Scroll target element not found');
            return;
        }

        const elementPosition = targetElement.offsetTop;
        const scrollPosition = elementPosition - offset;

        window.scrollTo({
            top: scrollPosition,
            behavior: behavior
        });
    }

    static buildUrl(baseUrl, params = {}) {
        const url = new URL(baseUrl, window.location.origin);

        Object.keys(params).forEach(key => {
            const value = params[key];
            if (value !== null && value !== undefined) {
                url.searchParams.set(key, value);
            }
        });

        return url.toString();
    }

    static getQueryParams() {
        const urlParams = new URLSearchParams(window.location.search);
        const params = {};

        for (const [key, value] of urlParams) {
            params[key] = value;
        }

        return params;
    }

    static updateUrl(params = {}, replaceState = false) {
        const newUrl = this.buildUrl(window.location.pathname, params);

        if (replaceState) {
            window.history.replaceState(null, '', newUrl);
        } else {
            window.history.pushState(null, '', newUrl);
        }
    }
}

window.Utils = Utils;

document.addEventListener('DOMContentLoaded', function() {
    const loadingElements = document.querySelectorAll('.loading-overlay');
    loadingElements.forEach(element => {
        if (element.style.display !== 'none') {
            element.style.display = 'none';
        }
    });

    const mainLoading = document.getElementById('loading-container');
    if (mainLoading) {
        mainLoading.style.display = 'none';
    }
});

window.addEventListener('load', function() {
    const loadingElements = document.querySelectorAll('.loading-overlay');
    loadingElements.forEach(element => {
        element.style.display = 'none';
    });
});