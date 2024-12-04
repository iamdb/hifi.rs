/** @type {import('tailwindcss').Config}*/
const config = {
	content: ['./src/**/*.{html,js,svelte,ts}'],
	plugins: [require('tailwindcss-safe-area')],
	future: {
		hoverOnlyWhenSupported: true
	}
};

module.exports = config;
