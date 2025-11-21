// Simple mobile navigation toggle
document.addEventListener('DOMContentLoaded', () => {
    // Check if mobile toggle exists
    const toggle = document.querySelector('.mobile-toggle');
    const menu = document.querySelector('.nav-menu');

    if (!toggle) {
        // Create mobile toggle if not exists
        const header = document.querySelector('.header .container');
        if (header) {
            const toggleBtn = document.createElement('button');
            toggleBtn.className = 'mobile-toggle';
            toggleBtn.innerHTML = '<span></span><span></span><span></span>';
            toggleBtn.style.cssText = 'display:none;flex-direction:column;gap:5px;background:transparent;border:none;padding:10px;';

            toggleBtn.querySelectorAll('span').forEach(span => {
                span.style.cssText = 'width:25px;height:3px;background:white;border-radius:3px;transition:all 0.3s;';
            });

            header.insertBefore(toggleBtn, menu);
        }
    }

    // Toggle functionality
    const mobileToggle = document.querySelector('.mobile-toggle');
    const navMenu = document.querySelector('.nav-menu');

    if (mobileToggle && navMenu) {
        mobileToggle.addEventListener('click', () => {
            mobileToggle.classList.toggle('active');
            navMenu.classList.toggle('active');
        });

        // Close on link click
        navMenu.querySelectorAll('a').forEach(link => {
            link.addEventListener('click', () => {
                mobileToggle.classList.remove('active');
                navMenu.classList.remove('active');
            });
        });
    }
});