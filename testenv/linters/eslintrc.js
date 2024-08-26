const js = require("/usr/lib/node_modules/eslint/node_modules/@eslint/js/src/index.js");
const globals = require("/usr/lib/node_modules/eslint/node_modules/@eslint/eslintrc/node_modules/globals/index.js");
const parser = require("/usr/lib/node_modules/@babel/eslint-parser/lib/index.cjs");

module.exports = [
	js.configs.recommended,

	{
		files: ["**/*.js"],
		languageOptions: {
			globals: globals.browser,
			ecmaVersion: 2015,
			parser: parser,
			parserOptions: {
				ecmaVersion: 2025,
				sourceType: "module",
				allowImportExportEverywhere: true,
				requireConfigFile: false,
			},
		},
	},

	{
		rules: {
			indent: [
				"error",
				"tab",
				{SwitchCase: 1},
			],
			"linebreak-style": [
				"error",
				"unix",
			],
			quotes: [
				"error",
				"double",
			],
			"quote-props": [
				"error",
				"always",
			],
			"semi": [
				"error",
				"always",
			],
			"comma-dangle": [
				"error",
				"always-multiline",
			],
			"no-unused-vars": [
				"error",
				{vars: "local", args: "after-used"},
			],
		},
	},

];
