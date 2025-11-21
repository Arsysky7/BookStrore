-- /pdf-bookstore/database/seeds/categories.sql

INSERT INTO categories (id, name, slug, description) VALUES
(gen_random_uuid(), 'Programming', 'programming', 'Programming languages, frameworks, and software development'),
(gen_random_uuid(), 'Business', 'business', 'Business strategy, entrepreneurship, and management'),
(gen_random_uuid(), 'Design', 'design', 'UI/UX design, graphic design, and creative arts'),
(gen_random_uuid(), 'Marketing', 'marketing', 'Digital marketing, SEO, and growth strategies'),
(gen_random_uuid(), 'Fiction', 'fiction', 'Novels, short stories, and creative writing'),
(gen_random_uuid(), 'Non-Fiction', 'non-fiction', 'Educational, biographical, and factual content'),
(gen_random_uuid(), 'Technology', 'technology', 'Latest tech trends, AI, and innovation'),
(gen_random_uuid(), 'Health', 'health', 'Wellness, fitness, and medical knowledge'),
(gen_random_uuid(), 'Education', 'education', 'Learning resources and academic materials'),
(gen_random_uuid(), 'Finance', 'finance', 'Personal finance, investing, and economics');