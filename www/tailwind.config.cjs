const defaultTheme = require('tailwindcss/defaultTheme')

/** @type {import('tailwindcss').Config}*/
const config = {
  content: [
    './src/**/*.{html,js,svelte,ts}',
  ],

  theme: {
    extend: {
      fontFamily: {
        serif: ['Bitter', ...defaultTheme.fontFamily.serif],
        sans: ['HandJet', ...defaultTheme.fontFamily.sans]
      }
    }
  },

  plugins: [
  ]
};

module.exports = config;
