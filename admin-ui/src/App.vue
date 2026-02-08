<template>
  <v-app>
    <v-navigation-drawer v-model="drawer">
      <RouterLink to="/">Home</RouterLink>
      <RouterLink to="/about">About</RouterLink>
      <RouterLink to="/banned-stickers">Banned Stickers</RouterLink>
    </v-navigation-drawer>

    <v-app-bar>
      <v-app-bar-nav-icon @click="drawer = !drawer"></v-app-bar-nav-icon>

      <v-app-bar-title>Application</v-app-bar-title>
    </v-app-bar>

    <v-main>
      <router-view />
    </v-main>
  </v-app>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { RouterLink, RouterView } from 'vue-router'

const drawer = ref<null | boolean>(null)
</script>

<script lang="ts">
export default {
  data: () => ({ drawer: null }),
}
</script>
