import { getCollection } from 'astro:content';
import type { APIRoute } from 'astro';

export const GET: APIRoute = async ({ site }) => {
  const guides = await getCollection('guides');
  const paths = ['/', '/docs/', ...guides.map((guide) => `/docs/${guide.id}/`)];
  const urls = paths.map((path) => `<url><loc>${new URL(path, site).href}</loc></url>`).join('');
  return new Response(
    `<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${urls}</urlset>`,
    {
      headers: { 'Content-Type': 'application/xml; charset=utf-8' },
    },
  );
};
