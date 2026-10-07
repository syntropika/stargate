import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';
import { guides } from './lib/guides';

export const collections = {
  guides: defineCollection({
    loader: glob({ pattern: guides.map((guide) => `${guide.id}.md`), base: '../docs' }),
  }),
};
